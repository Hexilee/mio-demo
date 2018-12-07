use crossbeam_channel::{unbounded, Sender};
use std::fs::File;
use std::io::Read;
use std::boxed::FnBox;
use mio::*;
use std::thread;

#[derive(Clone)]
pub struct Fs {
    task_sender: Sender<Task>,
}

pub struct FsHandler {
    io_worker: thread::JoinHandle<()>,
    executor: thread::JoinHandle<()>,
}

const FS_TOKEN: Token = Token(9);

pub fn fs_async() -> (Fs, FsHandler) {
    let (task_sender, task_receiver) = unbounded();
    let (result_sender, result_receiver) = unbounded();
    let poll = Poll::new().unwrap();
    let (registration, set_readiness) = Registration::new2();
    poll.register(&registration, FS_TOKEN, Ready::readable(), PollOpt::oneshot()).unwrap();
    let io_worker = thread::spawn(move || {
        let mut task_counter = 0;
        loop {
            match task_receiver.recv() {
                Ok(task) => {
                    task_counter += 1;
                    match task {
                        Task::Println(ref string) => println!("{}", string),
                        Task::Open(path, callback, fs) => {
                            result_sender
                                .send(TaskResult::Open(File::open(path).unwrap(), callback, fs))
                                .unwrap();
                            set_readiness.set_readiness(Ready::readable()).unwrap();
                        }
                        Task::ReadToString(mut file, callback, fs) => {
                            let mut value = String::new();
                            file.read_to_string(&mut value).unwrap();
                            result_sender
                                .send(TaskResult::ReadToString(value, callback, fs))
                                .unwrap();
                            set_readiness.set_readiness(Ready::readable()).unwrap();
                        }
                        Task::Exit => {
                            result_sender
                                .send(TaskResult::Exit)
                                .unwrap();
                            break;
                        }
                    }
                }
                Err(_) => {
                    break;
                }
            }
        };
        println!("task_counter: {}", task_counter);
    });

    let executor = thread::spawn(move || {
        let mut events = Events::with_capacity(1024);
        let mut total_event_counter = 0;
        let mut token_event_counter = 0;
        let mut loop_counter = 0;
        let mut result_counter = 0;
        'outer: loop {
            poll.poll(&mut events, None).unwrap();
            loop_counter += 1;
            for event in events.iter() {
                total_event_counter += 1;
                match event.token() {
                    FS_TOKEN => {
                        token_event_counter += 1;
                        loop {
                            match result_receiver.try_recv() {
                                Ok(result) => {
                                    result_counter += 1;
                                    match result {
                                        TaskResult::ReadToString(value, callback, fs) => callback(value, fs),
                                        TaskResult::Open(file, callback, fs) => callback(file, fs),
                                        TaskResult::Exit => break 'outer
                                    }
                                }
                                Err(_) => {
                                    break;
                                }
                            }
                        }
                        poll.reregister(&registration, FS_TOKEN, Ready::readable(), PollOpt::oneshot()).unwrap();
                    }

                    _ => unreachable!()
                }
            }
        };
        println!("total_event_counter: {}, token_event_counter: {}, loop_counter: {}, result_counter: {}", total_event_counter, token_event_counter, loop_counter, result_counter);
    });
    (Fs { task_sender }, FsHandler { io_worker, executor })
}

impl Fs {
    pub fn println(&self, string: String) {
        self.task_sender.send(Task::Println(string)).unwrap()
    }

    pub fn open<F: FnOnce(File, Fs) + Send + 'static>(&self, path: &str, callback: F) {
        self.task_sender.send(Task::Open(path.to_string(), Box::new(callback), self.clone())).unwrap()
    }

    pub fn read_to_string<F: FnOnce(String, Fs) + Send + 'static>(&self, file: File, callback: F) {
        self.task_sender.send(Task::ReadToString(file, Box::new(callback), self.clone())).unwrap()
    }

    pub fn close(&self) {
        self.task_sender.send(Task::Exit).unwrap()
    }
}

impl FsHandler {
    pub fn join(self) {
        self.io_worker.join().unwrap();
        self.executor.join().unwrap();
    }
}

type FileCallback = Box<FnBox(File, Fs) + Send>;
type StringCallback = Box<FnBox(String, Fs) + Send>;

pub enum Task {
    Exit,
    Println(String),
    Open(String, FileCallback, Fs),
    ReadToString(File, StringCallback, Fs),
}

pub enum TaskResult {
    Exit,
    Open(File, FileCallback, Fs),
    ReadToString(String, StringCallback, Fs),
}

const TEST_FILE_VALUE: &str = "Hello, World!";

#[test]
fn test_fs() {
    let (fs, fs_handler) = fs_async();
    fs.open("./src/test.txt", |file, fs| {
        fs.read_to_string(file, |value, fs| {
            assert_eq!(TEST_FILE_VALUE, &value);
            fs.println(value);
            fs.close();
        })
    });
    fs_handler.join();
}