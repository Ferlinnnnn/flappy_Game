mod obstacle;
mod player;
mod sound;

use bracket_lib::prelude::*;
use image::*;
use obstacle::Obstacle;
use player::Player;
use sound::{start_sound_thread, start_bgm_thread, SoundEffect};
use std::sync::mpsc::Sender;

// 按钮动作枚举
#[derive(Clone, Copy)]
enum ButtonAction {
    Play,
    Quit,
    ToggleAudio,
    ToggleMusic,
    Restart,
}

// 按钮结构体
struct Button {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text: String,
    action: ButtonAction,
    hover: bool,
}

impl Button {
    fn new(x: i32, y: i32, width: i32, height: i32, text: String, action: ButtonAction) -> Self {
        Button {
            x,
            y,
            width,
            height,
            text,
            action,
            hover: false,
        }
    }

    fn render(&self, ctx: &mut BTerm) {
        let bg_color = if self.hover { YELLOW } else { WHITE };
        let fg_color = if self.hover { BLACK } else { BLACK };
        
        // 绘制按钮背景
        for dx in 0..self.width {
            for dy in 0..self.height {
                ctx.set_bg(self.x + dx, self.y + dy, bg_color);
                ctx.set(self.x + dx, self.y + dy, fg_color, bg_color, to_cp437(' '));
            }
        }
        
        // 绘制按钮边框
        for dx in 0..self.width {
            ctx.set(self.x + dx, self.y, BLACK, bg_color, to_cp437('─'));
            ctx.set(self.x + dx, self.y + self.height - 1, BLACK, bg_color, to_cp437('─'));
        }
        for dy in 0..self.height {
            ctx.set(self.x, self.y + dy, BLACK, bg_color, to_cp437('│'));
            ctx.set(self.x + self.width - 1, self.y + dy, BLACK, bg_color, to_cp437('│'));
        }
        
        // 绘制按钮文字
        let text_x = self.x + (self.width - self.text.len() as i32) / 2;
        let text_y = self.y + self.height / 2;
        ctx.print(text_x, text_y, &self.text);
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

enum GameMode {
    Menu,
    Playing,
    End,
}

/// 游戏屏幕宽度
const SCREEN_WIDTH: i32 = 80;
/// 游戏屏幕高度
const SCREEN_HEIGHT: i32 = 50;
/// 每隔75毫秒做一些事情
const FRAME_DURATION: f32 = 75.0;
/// 初始障碍物数量
const INITIAL_OBSTACLES: usize = 3;
/// 障碍物间隔
const OBSTACLE_INTERVAL: i32 = 30;

struct State {
    player: Player,
    frame_time: f32,
    mode: GameMode,
    obstacles: Vec<Obstacle>,
    score: i32, // 分数
    sound_tx: Sender<SoundEffect>,
    audio_enabled: bool, // 音效开关
    music_enabled: bool, // 音乐开关
    bgm_play_tx: Sender<bool>, // 播放背景音乐
    bgm_stop_tx: Sender<bool>, // 停止背景音乐
    buttons: Vec<Button>, // 按钮列表
    last_obstacle_gap_y: Option<i32>, // 最后一个障碍物的中心点位置
}

impl State {
    fn new() -> Self {
        let sound_tx = start_sound_thread();
        let (bgm_play_tx, bgm_stop_tx) = start_bgm_thread();

        let mut obstacles = Vec::new();
        let mut x = SCREEN_WIDTH;
        for _ in 0..INITIAL_OBSTACLES {
            obstacles.push(Obstacle::new(x, 0));
            x += OBSTACLE_INTERVAL;
        }
        
        State {
            player: Player::new(5, 25),
            frame_time: 0.0,
            mode: GameMode::Menu,
            obstacles,
            score: 0,
            sound_tx,
            audio_enabled: true,
            music_enabled: true,
            bgm_play_tx,
            bgm_stop_tx,
            buttons: Vec::new(),
            last_obstacle_gap_y: None,
        }
    }

    // 播放音效的辅助函数
    fn play_sound(&self, effect: SoundEffect) {
        if self.audio_enabled {
            let _ = self.sound_tx.send(effect);
        }
    }

    // 控制背景音乐的辅助函数
    fn set_music(&self, enable: bool) {
        if enable {
            let _ = self.bgm_play_tx.send(true);
        } else {
            let _ = self.bgm_stop_tx.send(true);
        }
    }

    // 创建受限制的障碍物
    fn create_obstacle_with_constraint(&mut self, x: i32, score: i32) -> Obstacle {
        let mut random = RandomNumberGenerator::new();
        let size = i32::max(5, 20 - score); // 洞口最小为5
        let half_size = size / 2;
        let min_gap_y = half_size + 2;
        let max_gap_y = SCREEN_HEIGHT - half_size - 2;
        
        let gap_y = if let Some(last_gap_y) = self.last_obstacle_gap_y {
            // 限制与上一个障碍物的差距不超过2/3屏幕高度
            let max_diff = (SCREEN_HEIGHT * 2) / 3;
            let min_allowed = i32::max(min_gap_y, last_gap_y - max_diff);
            let max_allowed = i32::min(max_gap_y, last_gap_y + max_diff);
            random.range(min_allowed, max_allowed)
        } else {
            // 第一个障碍物，随机生成
            random.range(min_gap_y, max_gap_y)
        };
        
        self.last_obstacle_gap_y = Some(gap_y);
        Obstacle { x, gap_y, size }
    }

    // 创建主菜单按钮
    fn create_menu_buttons(&mut self) {
        self.buttons.clear();
        self.buttons.push(Button::new(30, 15, 20, 3, "Play Game".to_string(), ButtonAction::Play));
        self.buttons.push(Button::new(30, 20, 20, 3, "Quit Game".to_string(), ButtonAction::Quit));
        self.buttons.push(Button::new(30, 25, 20, 3, format!("Audio: {}", if self.audio_enabled { "ON" } else { "OFF" }), ButtonAction::ToggleAudio));
        self.buttons.push(Button::new(30, 30, 20, 3, format!("Music: {}", if self.music_enabled { "ON" } else { "OFF" }), ButtonAction::ToggleMusic));
    }

    // 创建游戏结束按钮
    fn create_end_buttons(&mut self) {
        self.buttons.clear();
        self.buttons.push(Button::new(30, 20, 20, 3, "Play Again".to_string(), ButtonAction::Restart));
        self.buttons.push(Button::new(30, 25, 20, 3, "Quit Game".to_string(), ButtonAction::Quit));
        self.buttons.push(Button::new(30, 30, 20, 3, format!("Audio: {}", if self.audio_enabled { "ON" } else { "OFF" }), ButtonAction::ToggleAudio));
        self.buttons.push(Button::new(30, 35, 20, 3, format!("Music: {}", if self.music_enabled { "ON" } else { "OFF" }), ButtonAction::ToggleMusic));
    }

    // 处理按钮点击
    fn handle_button_click(&mut self, action: ButtonAction, ctx: &mut BTerm) {
        match action {
            ButtonAction::Play => self.restart(),
            ButtonAction::Quit => ctx.quitting = true,
            ButtonAction::ToggleAudio => {
                self.audio_enabled = !self.audio_enabled;
                // 重新创建按钮以更新文本
                match self.mode {
                    GameMode::Menu => self.create_menu_buttons(),
                    GameMode::End => self.create_end_buttons(),
                    _ => {}
                }
            },
            ButtonAction::ToggleMusic => {
                self.music_enabled = !self.music_enabled;
                self.set_music(self.music_enabled);
                // 重新创建按钮以更新文本
                match self.mode {
                    GameMode::Menu => self.create_menu_buttons(),
                    GameMode::End => self.create_end_buttons(),
                    _ => {}
                }
            },
            ButtonAction::Restart => self.restart(),
        }
    }

    // 处理鼠标事件
    fn handle_mouse(&mut self, ctx: &mut BTerm) {
        let (mouse_x, mouse_y) = ctx.mouse_pos();
        
        // 重置所有按钮的悬停状态
        for button in &mut self.buttons {
            button.hover = button.contains_point(mouse_x, mouse_y);
        }
        
        // 检查鼠标点击
        if ctx.left_click {
            for button in &self.buttons {
                if button.contains_point(mouse_x, mouse_y) {
                    self.handle_button_click(button.action, ctx);
                    break;
                }
            }
        }
    }

    fn main_menu(&mut self, ctx: &mut BTerm) {
        // 清空屏幕
        ctx.cls();
        
        // 创建按钮（如果还没有创建）
        if self.buttons.is_empty() {
            self.create_menu_buttons();
        }
        
        // 绘制标题
        ctx.print_centered(5, "Welcome to Flappy Dragon！");
        ctx.print_centered(7, "Click buttons or use keyboard shortcuts:");
        ctx.print_centered(8, "P - Play, Q - Quit, M - Audio, B - Music");
        
        // 绘制背景
        self.set_background(ctx, "assets/background.png");
        
        // 绘制按钮
        for button in &self.buttons {
            button.render(ctx);
        }
        
        // 处理鼠标事件
        self.handle_mouse(ctx);
        
        // 处理键盘事件（保持向后兼容）
        if let Some(key) = ctx.key {
            match key {
                VirtualKeyCode::P => self.restart(),
                VirtualKeyCode::Q => ctx.quitting = true,
                VirtualKeyCode::M => {
                    self.audio_enabled = !self.audio_enabled;
                    self.create_menu_buttons(); // 更新按钮文本
                },
                VirtualKeyCode::B => {
                    self.music_enabled = !self.music_enabled;
                    self.set_music(self.music_enabled);
                    self.create_menu_buttons(); // 更新按钮文本
                },
                _ => {}
            }
        }
    }

    fn play(&mut self, ctx: &mut BTerm) {
        ctx.cls_bg(NAVY);
        // frame_time_ms 记录了每次调用tick所经过的时间
        self.frame_time += ctx.frame_time_ms;
        // 向前移动并且重力增加
        if self.frame_time > FRAME_DURATION {
            self.frame_time = 0.0;
            self.player.gravity_and_move();
        }
        // 空格触发，往上飞
        if let Some(VirtualKeyCode::Space) = ctx.key {
            self.player.flap();
            self.play_sound(SoundEffect::Flap);
        }
        // 音频切换
        if let Some(VirtualKeyCode::M) = ctx.key {
            self.audio_enabled = !self.audio_enabled;
        }
        // 音乐切换
        if let Some(VirtualKeyCode::B) = ctx.key {
            self.music_enabled = !self.music_enabled;
            self.set_music(self.music_enabled);
        }
        // 渲染
        self.player.render(ctx);
        ctx.print(0, 0, "Press Space to Flap");
        ctx.print(0, 1, &format!("Score: {}", self.score));
        ctx.print(0, 2, &format!("Audio: {}  Music: {}", if self.audio_enabled { "ON" } else { "OFF" }, if self.music_enabled { "ON" } else { "OFF" }));

        // 渲染障碍物
        for obstacle in &mut self.obstacles {
            obstacle.render(ctx, self.player.x);
        }

        // 检查是否越过障碍物
        let mut passed = None;
        let mut hit_obstacle = false;
        for (i, obstacle) in self.obstacles.iter_mut().enumerate() {
            if self.player.x > obstacle.x {
                passed = Some(i);
            }
            if obstacle.hit_obstacle(&self.player) {
                hit_obstacle = true;
            }
        }
        
        // 处理碰撞
        if hit_obstacle {
            self.mode = GameMode::End;
            self.play_sound(SoundEffect::Hit);
        }
        
        if let Some(i) = passed {
            self.score += 1;
            // 新障碍物x取当前所有障碍物最大x+OBSTACLE_INTERVAL
            let max_x = self.obstacles.iter().map(|o| o.x).max().unwrap_or(SCREEN_WIDTH);
            let new_x = max_x + OBSTACLE_INTERVAL;
            self.obstacles[i] = self.create_obstacle_with_constraint(new_x, self.score);
        }

        // 如果y 大于游戏高度，就是坠地，则游戏结束
        if self.player.y > SCREEN_HEIGHT {
            self.mode = GameMode::End;
            self.play_sound(SoundEffect::GameOver);
        }
    }

    fn dead(&mut self, ctx: &mut BTerm) {
        // 清空屏幕
        ctx.cls();
        
        // 创建按钮（如果还没有创建）
        if self.buttons.is_empty() {
            self.create_end_buttons();
        }
        
        // 绘制游戏结束信息
        ctx.print_centered(5, "You are dead！");
        ctx.print_centered(6, &format!("You earned {} points", self.score));
        ctx.print_centered(8, "Click buttons or use keyboard shortcuts:");
        ctx.print_centered(9, "P - Play Again, Q - Quit, M - Audio, B - Music");
        
        // 绘制按钮
        for button in &self.buttons {
            button.render(ctx);
        }
        
        // 处理鼠标事件
        self.handle_mouse(ctx);

        // 处理键盘事件（保持向后兼容）
        if let Some(key) = ctx.key {
            match key {
                VirtualKeyCode::P => self.restart(),
                VirtualKeyCode::Q => ctx.quitting = true,
                VirtualKeyCode::M => {
                    self.audio_enabled = !self.audio_enabled;
                    self.create_end_buttons(); // 更新按钮文本
                },
                VirtualKeyCode::B => {
                    self.music_enabled = !self.music_enabled;
                    self.set_music(self.music_enabled);
                    self.create_end_buttons(); // 更新按钮文本
                },
                _ => {}
            }
        }
    }

    fn restart(&mut self) {
        self.player = Player::new(5, 25);
        self.frame_time = 0.0;
        self.mode = GameMode::Playing;
        self.obstacles.clear();
        self.last_obstacle_gap_y = None; // 重置障碍物位置跟踪
        
        let mut x = SCREEN_WIDTH;
        for _ in 0..INITIAL_OBSTACLES {
            let obstacle = self.create_obstacle_with_constraint(x, 0);
            self.obstacles.push(obstacle);
            x += OBSTACLE_INTERVAL;
        }
        self.score = 0;
        self.buttons.clear(); // 清空按钮列表
        
        // 如果音乐开启，播放背景音乐
        if self.music_enabled {
            self.set_music(true);
        }
    }

    pub fn set_background(&mut self, ctx: &mut BTerm, url: &str) {
        let img = image::open(url).unwrap();
        let (img_width, img_height) = img.dimensions();
        // Draw image to console
        for x in 0..img_width {
            for y in 0..img_height {
                let pixel = img.get_pixel(x, y);
                ctx.set_bg(x as i32, y as i32, (pixel[0], pixel[1], pixel[2]));
            }
        }
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        match self.mode {
            GameMode::Menu => self.main_menu(ctx),
            GameMode::Playing => self.play(ctx),
            GameMode::End => self.dead(ctx),
        }
    }
}

fn main() -> BError {
    let context = BTermBuilder::simple80x50()
        .with_title("Flappy Dragon")
        .build()?;
    main_loop(context, State::new())
}