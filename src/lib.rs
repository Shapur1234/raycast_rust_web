use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const SCREEN_WIDTH: usize = 1280;
const SCREEN_HEIGHT: usize = 760;
const FOV: u32 = 80;
const MOVEMENT_SPEED_MODIFIER: f32 = 0.05;
const INTERNAL_RESOLUTION_MULTIPLIER: u32 = 16;

static mut GAME_RUNNING: bool = false;
static mut POINTER_SHOULD_BE_LOCKED: bool = false;
static mut PLAYER_CAMERA: Camera = Camera {
    pos: Point { x: 1.5, y: 1.5 },
    rotation: Rotation { degree: 0.0 },
};

// --------------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

macro_rules! console_log {
        ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
    }

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .expect("no global `window` exists")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u32,
}

impl Rect {
    fn fit_self_to_screen(&mut self) {
        if self.x >= SCREEN_WIDTH {
            self.x = SCREEN_WIDTH - 1
        }
        if self.y >= SCREEN_HEIGHT {
            self.y = SCREEN_HEIGHT - 1
        }
        if self.x + self.width >= SCREEN_WIDTH {
            self.width = SCREEN_WIDTH - self.x
        }
        if self.y + self.height >= SCREEN_HEIGHT {
            self.height = SCREEN_HEIGHT - self.y
        }
    }
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Level {
    layout: Vec<Vec<Tile>>,
    width: usize,
    height: usize,
}

impl Level {
    fn new(layout_input: Vec<Vec<Tile>>) -> Level {
        Level {
            width: layout_input[0].len(),
            height: layout_input.len(),
            layout: layout_input,
        }
    }
    fn get_tile(&self, point: &Point) -> &Tile {
        if (point.x >= 0.0 && point.x < (self.width as f32))
            && (point.y >= 0.0 && point.y < (self.height as f32))
        {
            &self.layout[point.y as usize][point.x as usize]
        }
        // TO FIX
        else {
            &self.layout[1][1]
        }
    }
    fn is_in_level(&self, point: &Point) -> bool {
        if (point.x < 0.0 || point.x > self.width as f32)
            || (point.y < 0.0 || point.y > self.height as f32)
        {
            false
        } else {
            true
        }
    }
}

#[derive(Debug, Clone)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    fn new(red: u8, green: u8, blue: u8) -> Color {
        Color {
            r: red,
            g: green,
            b: blue,
        }
    }
    fn get_color_from_distance(&self, distance: f32) -> u32 {
        rgb_to_u32(
            ((self.r as f32 / (distance * 0.6)) as u8).clamp(self.r / 10, self.r),
            ((self.g as f32 / (distance * 0.6)) as u8).clamp(self.g / 10, self.g),
            ((self.b as f32 / (distance * 0.6)) as u8).clamp(self.b / 10, self.b),
        )
    }
    fn to_u32(&self) -> u32 {
        rgb_to_u32(self.r, self.g, self.b)
    }
}

enum TileType {
    Air,
    Stone,
    Yellow,
}

#[derive(Debug, Clone)]
struct Tile {
    solid: bool,
    transparent: bool,
    base_color: Color,
}

impl Tile {
    fn new(tile_type: TileType) -> Tile {
        Tile {
            solid: match &tile_type {
                TileType::Air => false,
                _other => true,
            },
            transparent: match &tile_type {
                TileType::Air => false,
                _other => true,
            },
            base_color: match &tile_type {
                TileType::Air => Color::new(0xFF, 0xFF, 0xFF),
                TileType::Stone => Color::new(0x38, 0x36, 0x30),
                TileType::Yellow => Color::new(0xCC, 0xFF, 0x00),
            },
        }
    }
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct InputInfo {
    forward: bool,
    backward: bool,
    right: bool,
    left: bool,
    rot_right: bool,
    rot_left: bool,
}

#[derive(Debug, Clone)]
struct Camera {
    pos: Point,
    rotation: Rotation,
}

impl Camera {
    fn new(pos: Point) -> Camera {
        Camera {
            pos: Point::new(pos.x, pos.y),
            rotation: Rotation::new(0.0),
        }
    }
    fn get_angles_to_cast(&self) -> Vec<Rotation> {
        let mut output: Vec<Rotation> = Vec::new();
        for i in ((self.rotation.degree as i32) - ((FOV / 2) as i32))
            ..((self.rotation.degree as i32) + ((FOV / 2) as i32))
        {
            for x in 0..INTERNAL_RESOLUTION_MULTIPLIER {
                output.push(Rotation::new(
                    i as f32 + (1.0 / INTERNAL_RESOLUTION_MULTIPLIER as f32) * x as f32,
                ));
            }
        }
        output
    }
    fn update_from_input(&mut self, level: &Level, input: InputInfo) {
        let mut x_change: f32 = 0.0;
        let mut y_change: f32 = 0.0;

        if input.rot_right {
            self.rotation.mod_value(10.0)
        }
        if input.rot_left {
            self.rotation.mod_value(-10.0)
        }

        if input.forward {
            x_change += MOVEMENT_SPEED_MODIFIER * self.rotation.to_rad().cos();
            y_change += MOVEMENT_SPEED_MODIFIER * self.rotation.to_rad().sin();
        }
        if input.backward {
            x_change -= MOVEMENT_SPEED_MODIFIER * self.rotation.to_rad().cos();
            y_change -= MOVEMENT_SPEED_MODIFIER * self.rotation.to_rad().sin();
        }
        if input.right {
            x_change += MOVEMENT_SPEED_MODIFIER * (self.rotation.to_rad() + 1.570796).cos();
            y_change += MOVEMENT_SPEED_MODIFIER * (self.rotation.to_rad() + 1.570796).sin();
        }
        if input.left {
            x_change += MOVEMENT_SPEED_MODIFIER * (self.rotation.to_rad() - 1.570796).cos();
            y_change += MOVEMENT_SPEED_MODIFIER * (self.rotation.to_rad() - 1.570796).sin();
        }

        if !level
            .get_tile(&Point {
                x: self.pos.x + x_change,
                y: self.pos.y,
            })
            .solid
        {
            self.pos.x += x_change
        }
        if !level
            .get_tile(&Point {
                x: self.pos.x,
                y: self.pos.y + y_change,
            })
            .solid
        {
            self.pos.y += y_change
        }
    }
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Rotation {
    degree: f32,
}

impl Rotation {
    fn new(initial_value: f32) -> Rotation {
        Rotation {
            degree: clamp_degrees(initial_value),
        }
    }
    fn mod_value(&mut self, value: f32) {
        self.degree = clamp_degrees(self.degree + value);
    }
    fn to_rad(&self) -> f32 {
        self.degree * (std::f32::consts::PI / 180.0)
    }
}

fn clamp_degrees(value: f32) -> f32 {
    let mut degree_temp = value;
    while degree_temp < 0.0 {
        degree_temp += 360.0;
    }
    while degree_temp > 360.0 {
        degree_temp -= 360.0;
    }
    degree_temp
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn new(x_val: f32, y_val: f32) -> Point {
        Point { x: x_val, y: y_val }
    }
}

// --------------------------------------------------------------------------------

fn draw_rect(dest: &web_sys::CanvasRenderingContext2d, rect: Rect) {
    dest.set_fill_style(&JsValue::from_str(&format!("#{:06x}", rect.color)));
    dest.fill_rect(
        rect.x as f64,
        rect.y as f64,
        rect.width as f64,
        rect.height as f64,
    )
}
fn draw_not_running(dest: &web_sys::CanvasRenderingContext2d) {
    const NOT_RUNNING_RECTS: [Rect; 1] = [Rect {
        x: 0,
        y: 0,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        color: 0x00000000,
    }];

    for rect in NOT_RUNNING_RECTS {
        draw_rect(&dest, rect);
    }
}
fn draw_background(dest: &web_sys::CanvasRenderingContext2d) {
    const BACKGROUND_RECTS: [Rect; 2] = [
        Rect {
            x: 0,
            y: 0,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT / 2,
            color: 0x0000ffff,
        },
        Rect {
            x: 0,
            y: SCREEN_HEIGHT / 2,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT / 2,
            color: 0x008b4513,
        },
    ];

    for rect in BACKGROUND_RECTS {
        draw_rect(&dest, rect);
    }
}
fn draw_minimap(dest: &web_sys::CanvasRenderingContext2d, camera: &Camera, level: &Level) {
    const TILE_SIZE: usize = 16;

    for y in 0..level.height {
        for x in 0..level.width {
            draw_rect(
                dest,
                Rect {
                    x: SCREEN_WIDTH - ((x + 2) * TILE_SIZE),
                    y: (y + 1) * TILE_SIZE,
                    width: TILE_SIZE,
                    height: TILE_SIZE,
                    color: level
                        .get_tile(&Point::new(x as f32, y as f32))
                        .base_color
                        .to_u32(),
                },
            );
        }
    }

    if level.is_in_level(&camera.pos) {
        draw_rect(
            dest,
            Rect {
                x: SCREEN_WIDTH
                    - ((camera.pos.x + 1.0) * (TILE_SIZE as f32)) as usize
                    - TILE_SIZE / 4,
                y: ((camera.pos.y + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 4,
                width: TILE_SIZE / 2,
                height: TILE_SIZE / 2,
                color: 0x0ff0000,
            },
        );
    }

    let cast_result: Point = cast_ray(&camera.pos, &camera.rotation, level);
    if level.is_in_level(&cast_result) {
        draw_rect(
            dest,
            Rect {
                x: SCREEN_WIDTH
                    - ((cast_result.x + 1.0) * (TILE_SIZE as f32)) as usize
                    - TILE_SIZE / 8,
                y: ((cast_result.y + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 8,
                width: TILE_SIZE / 4,
                height: TILE_SIZE / 4,
                color: 0x00000ff,
            },
        );
    }
}
fn draw_walls(dest: &web_sys::CanvasRenderingContext2d, camera: &Camera, level: &Level) {
    const SLICE_WIDTH: usize = SCREEN_WIDTH / (FOV * INTERNAL_RESOLUTION_MULTIPLIER) as usize;
    let mut wall_distances: Vec<f32> = vec![];
    let mut wall_base_colors: Vec<&Color> = vec![];

    for angle in camera.get_angles_to_cast() {
        let cast_result: Point = cast_ray(&camera.pos, &angle, &level);
        wall_distances.push(calc_distance_between_points(&camera.pos, &cast_result));
        wall_base_colors.push(&level.get_tile(&cast_result).base_color)
    }

    let mut loop_count: usize = 0;
    for wall_distance in wall_distances {
        let wall_height: f32 = (SCREEN_HEIGHT as f32 * 0.8) / wall_distance;

        let mut rect_to_draw: Rect = Rect {
            x: SLICE_WIDTH * loop_count,
            y: ((SCREEN_HEIGHT as f32 / 2.0) - (wall_height / 2.0)) as usize,
            width: SLICE_WIDTH + 1,
            height: wall_height as usize,
            color: wall_base_colors[loop_count].get_color_from_distance(wall_distance),
        };
        rect_to_draw.fit_self_to_screen();
        draw_rect(dest, rect_to_draw);
        loop_count += 1;
    }
}
fn calc_distance_between_points(point1: &Point, point2: &Point) -> f32 {
    ((point1.x - point2.x).powf(2.0) + (point1.y - point2.y).powf(2.0)).powf(0.5)
}
fn cast_ray(pos: &Point, rotation: &Rotation, level: &Level) -> Point {
    const MAX_DISTANCE: f32 = 10.0;
    const STEP: f32 = 0.01;

    let mut distance_travelled: f32 = 0.0;
    let mut point_to_check = Point::new(pos.x, pos.y);
    let mut has_hit: bool = false;
    while distance_travelled.abs() < MAX_DISTANCE {
        distance_travelled += STEP;

        point_to_check = Point::new(
            pos.x + (distance_travelled * rotation.to_rad().cos()),
            pos.y + (distance_travelled * rotation.to_rad().sin()),
        );
        if level.get_tile(&point_to_check).transparent {
            has_hit = true;
            break;
        }
    }
    if has_hit {
        point_to_check
    } else {
        Point::new(-1000.0, -1000.0)
    }
}
fn rgb_to_u32(red: u8, green: u8, blue: u8) -> u32 {
    (0x10000 * red as u32) + (0x100 * green as u32) + (blue as u32)
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let game_cavas_html = document
        .get_element_by_id("game_canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();
    let game_canvas = game_cavas_html
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    game_cavas_html.set_width(SCREEN_WIDTH as u32);
    game_cavas_html.set_height(SCREEN_HEIGHT as u32);

    let current_level: Level = Level::new(vec![
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Air),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
        ],
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Stone),
        ],
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Air),
            Tile::new(TileType::Yellow),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Stone),
        ],
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Air),
            Tile::new(TileType::Stone),
        ],
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Air),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
        ],
        vec![
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
            Tile::new(TileType::Stone),
        ],
    ]);

    // Keyboard input
    {
        let current_level_2 = current_level.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| unsafe {
            if GAME_RUNNING {
                let pressed_key = event.key_code();
                PLAYER_CAMERA.update_from_input(
                    &current_level_2,
                    InputInfo {
                        forward: if pressed_key == 87 { true } else { false },
                        backward: if pressed_key == 83 { true } else { false },
                        right: if pressed_key == 68 { true } else { false },
                        left: if pressed_key == 65 { true } else { false },
                        rot_right: if pressed_key == 69 { true } else { false },
                        rot_left: if pressed_key == 81 { true } else { false },
                    },
                );
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Mouse input
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| unsafe {
            if GAME_RUNNING {
                PLAYER_CAMERA.rotation.degree +=
                    ((event.movement_x() * 10) as f32) * MOVEMENT_SPEED_MODIFIER;
            }
        }) as Box<dyn FnMut(_)>);
        game_cavas_html
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Mouse click
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| unsafe {
            if !GAME_RUNNING {
                web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id("game_canvas")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .map_err(|_| ())
                    .unwrap()
                    .request_pointer_lock();

                GAME_RUNNING = true;
                POINTER_SHOULD_BE_LOCKED = true;
            }
            console_log!(
                "GAME_RUNNING: {:?},  POINTER_LOCK: {:?}",
                GAME_RUNNING,
                POINTER_SHOULD_BE_LOCKED
            );
        }) as Box<dyn FnMut(_)>);
        game_cavas_html
            .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Pointerlock exit
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| unsafe {
            if POINTER_SHOULD_BE_LOCKED {
                POINTER_SHOULD_BE_LOCKED = false;
            } else {
                GAME_RUNNING = false;
            }
            console_log!(
                "GAME_RUNNING: {:?},  POINTER_LOCK: {:?}",
                GAME_RUNNING,
                POINTER_SHOULD_BE_LOCKED
            );
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback(
            "pointerlockchange",
            closure.as_ref().unchecked_ref(),
        )?;
        closure.forget();
    }
    // Pointerlock error
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| unsafe {
            false;
            GAME_RUNNING = false;
            console_log!(
                "GAME_RUNNING: {:?},  POINTER_LOCK: {:?}",
                GAME_RUNNING,
                POINTER_SHOULD_BE_LOCKED
            );
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback(
            "pointerlockerror",
            closure.as_ref().unchecked_ref(),
        )?;
        closure.forget();
    }
    // Game loop
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        unsafe {
            if GAME_RUNNING {
                draw_background(&game_canvas);
                draw_walls(&game_canvas, &PLAYER_CAMERA, &current_level);
                draw_minimap(&game_canvas, &PLAYER_CAMERA, &current_level);
            } else {
                draw_not_running(&game_canvas);
            }
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}
