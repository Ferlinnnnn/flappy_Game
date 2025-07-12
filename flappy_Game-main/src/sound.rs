use std::sync::mpsc::Sender;
use std::thread;

pub enum SoundEffect {
    Flap,
    Hit,
    GameOver,
    BGM, // 添加背景音乐
}

pub fn start_sound_thread() -> Sender<SoundEffect> {
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        // 初始化音频输出流
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(stream) => stream,
            Err(_) => {
                eprintln!("无法初始化音频输出，将使用控制台输出");
                // 如果音频初始化失败，回退到控制台输出
                loop {
                    if let Ok(effect) = rx.recv() {
                        match effect {
                            SoundEffect::Flap => println!("播放跳跃音效"),
                            SoundEffect::Hit => println!("播放撞击音效"),
                            SoundEffect::GameOver => println!("播放游戏结束音效"),
                            SoundEffect::BGM => println!("播放背景音乐"),
                        }
                    }
                }
            }
        };

        loop {
            if let Ok(effect) = rx.recv() {
                match effect {
                    SoundEffect::BGM => {
                        // 处理背景音乐
                        if let Ok(file) = File::open("assets/bgm.wav") {
                            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                if let Ok(sink) = Sink::try_new(&stream_handle) {
                                    // 背景音乐循环播放
                                    sink.append(source.repeat_infinite());
                                    sink.detach(); // 播放后自动释放
                                }
                            }
                        }
                    },
                    _ => {
                        // 处理其他音效
                        let file_path = match effect {
                            SoundEffect::Flap => "assets/flap.wav",
                            SoundEffect::Hit => "assets/hit.wav",
                            SoundEffect::GameOver => "assets/gameover.wav",
                            SoundEffect::BGM => unreachable!(), // 已经在上面处理了
                        };
                        
                        // 尝试播放音效文件
                        if let Ok(file) = File::open(file_path) {
                            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                if let Ok(sink) = Sink::try_new(&stream_handle) {
                                    sink.append(source);
                                    sink.detach(); // 播放后自动释放
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    tx
}

// 添加背景音乐控制功能
pub fn start_bgm_thread() -> (Sender<bool>, Sender<bool>) {
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;
    use std::sync::mpsc;

    let (play_tx, play_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();

    thread::spawn(move || {
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(stream) => stream,
            Err(_) => {
                eprintln!("无法初始化背景音乐音频输出");
                return;
            }
        };

        let mut current_sink: Option<Sink> = None;

        loop {
            // 检查播放信号
            if let Ok(_) = play_rx.try_recv() {
                // 停止当前播放
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }
                
                // 开始播放背景音乐
                if let Ok(file) = File::open("assets/bgm.wav") {
                    if let Ok(source) = Decoder::new(BufReader::new(file)) {
                        if let Ok(sink) = Sink::try_new(&stream_handle) {
                            sink.append(source.repeat_infinite());
                            current_sink = Some(sink);
                        }
                    }
                }
            }

            // 检查停止信号
            if let Ok(_) = stop_rx.try_recv() {
                if let Some(sink) = current_sink.take() {
                    sink.stop();
                }
            }

            // 短暂休眠避免CPU占用过高
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    (play_tx, stop_tx)
}