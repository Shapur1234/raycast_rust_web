use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const SCREEN_WIDTH: usize = 1280;
const SCREEN_HEIGHT: usize = 760;
const FOV: u32 = 80;
const MOVEMENT_SPEED_MODIFIER: f32 = 0.05;
const INTERNAL_RESOLUTION_MULTIPLIER: u32 = 16;
const RENDER_DISTANCE: f32 = 10.0;

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
    color: Color,
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
struct Texture {
    width: usize,
    height: usize,
    layout: Vec<Vec<usize>>,
    colors: Vec<Color>,
}

impl Texture {
    fn new(layout_input: Vec<Vec<usize>>, layout_color: Vec<Color>) -> Texture {
        Texture {
            width: layout_input[0].len(),
            height: layout_input.len(),
            layout: layout_input,
            colors: layout_color,
        }
    }
    fn get_color(&self, point: &Point) -> &Color {
        if (point.x >= 0.0 && point.x < (self.width as f32))
            && (point.y >= 0.0 && point.y < (self.height as f32))
        {
            &self.colors[self.layout[point.y as usize][point.x as usize]]
        }
        // TODO FIX
        else {
            &self.colors[self.layout[0][0]]
        }
    }
}

// --------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Level {
    layout: Vec<Vec<u8>>,
    all_tiles: Vec<Tile>,
    all_textures: Vec<Texture>,
    width: usize,
    height: usize,
}

impl Level {
    fn new(layout: Vec<Vec<u8>>, all_tiles: Vec<Tile>, all_textures: Vec<Texture>) -> Level {
        console_log!("{:?}", layout);
        Level {
            width: layout[0].len(),
            height: layout.len(),
            layout: layout,
            all_tiles: all_tiles,
            all_textures: all_textures,
        }
    }
    fn get_tile(&self, point: &Point) -> &Tile {
        if (point.x >= 0.0 && point.x < (self.width as f32))
            && (point.y >= 0.0 && point.y < (self.height as f32))
        {
            &self.all_tiles[self.layout[point.y as usize][point.x as usize] as usize]
        } else {
            &self.all_tiles[0]
        }
    }
    fn get_texture(&self, point: &Point) -> &Texture {
        &self.all_textures[self.get_tile(point).texture_index as usize]
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

#[derive(Debug, Clone, Copy)]
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
    fn distance_self(&mut self, distance: f32) {
        self.r = ((self.r as f32 / (distance * 0.6)) as u8).clamp(self.r / 10, self.r);
        self.g = ((self.g as f32 / (distance * 0.6)) as u8).clamp(self.g / 10, self.g);
        self.b = ((self.b as f32 / (distance * 0.6)) as u8).clamp(self.b / 10, self.b);
    }
    fn to_u32(&self) -> u32 {
        rgb_to_u32(self.r, self.g, self.b)
    }
}

enum TileType {
    Air,
    Brick,
}

#[derive(Debug, Clone)]
struct Tile {
    solid: bool,
    transparent: bool,
    texture_index: u8,
}

impl Tile {
    fn new(texture_index: u8, solid: bool, transparent: bool) -> Tile {
        Tile {
            texture_index: texture_index,
            solid: solid,
            transparent: transparent,
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
    dest.set_fill_style(&JsValue::from_str(&format!("#{:06x}", rect.color.to_u32())));
    dest.fill_rect(
        rect.x as f64,
        rect.y as f64,
        rect.width as f64,
        rect.height as f64,
    )
}
fn draw_rect_to_buffer(dest: &mut Vec<u8>, rect: &mut Rect) {
    rect.fit_self_to_screen();
    for y in 0..rect.height {
        for x in 0..rect.width {
            let pos: usize = (((rect.y + y) * SCREEN_WIDTH) + rect.x + x) * 4;
            dest[pos + 0] = rect.color.r;
            dest[pos + 1] = rect.color.g;
            dest[pos + 2] = rect.color.b;
            dest[pos + 3] = 255;
        }
    }
}
fn draw_rect_to_buffer_distanced(dest: &mut Vec<u8>, rect: &mut Rect, distance: f32) {
    rect.color.distance_self(distance);
    draw_rect_to_buffer(dest, rect);
}
fn draw_not_running(dest: &web_sys::CanvasRenderingContext2d) {
    const NOT_RUNNING_RECTS: [Rect; 1] = [Rect {
        x: 0,
        y: 0,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        color: Color { r: 0, g: 0, b: 0 },
    }];

    for rect in NOT_RUNNING_RECTS {
        draw_rect(&dest, rect);
    }
}
fn draw_background(dest: &mut Vec<u8>) {
    draw_rect_to_buffer(
        dest,
        &mut Rect {
            x: 0,
            y: 0,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
            color: Color::new(0x4f, 0x4f, 0x4f),
        },
    )
}
fn draw_minimap(dest: &mut Vec<u8>, camera: &Camera, level: &Level) {
    const TILE_SIZE: usize = 16;

    for y in 0..level.height {
        for x in 0..level.width {
            draw_rect_to_buffer(
                dest,
                &mut Rect {
                    x: SCREEN_WIDTH - ((x + 2) * TILE_SIZE),
                    y: (y + 1) * TILE_SIZE,
                    width: TILE_SIZE,
                    height: TILE_SIZE,
                    color: Color::new(0, 200, 0),
                },
            );
        }
    }

    if level.is_in_level(&camera.pos) {
        draw_rect_to_buffer(
            dest,
            &mut Rect {
                x: SCREEN_WIDTH
                    - ((camera.pos.x + 1.0) * (TILE_SIZE as f32)) as usize
                    - TILE_SIZE / 4,
                y: ((camera.pos.y + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 4,
                width: TILE_SIZE / 2,
                height: TILE_SIZE / 2,
                color: Color::new(255, 0, 0),
            },
        );
    }

    let cast_result: Point = cast_ray(&camera.pos, &camera.rotation, level);
    if level.is_in_level(&cast_result) {
        draw_rect_to_buffer(
            dest,
            &mut Rect {
                x: SCREEN_WIDTH
                    - ((cast_result.x + 1.0) * (TILE_SIZE as f32)) as usize
                    - TILE_SIZE / 8,
                y: ((cast_result.y + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 8,
                width: TILE_SIZE / 4,
                height: TILE_SIZE / 4,
                color: Color::new(0, 0, 255),
            },
        );
    }
}
fn draw_walls_to_buffer(dest: &mut Vec<u8>, camera: &Camera, level: &Level) {
    const SLICE_WIDTH: usize = SCREEN_WIDTH / (FOV * INTERNAL_RESOLUTION_MULTIPLIER) as usize;
    let mut wall_distances: Vec<f32> = vec![];
    let mut cast_results: Vec<Point> = vec![];

    for angle in camera.get_angles_to_cast() {
        cast_results.push(cast_ray(&camera.pos, &angle, &level));
    }

    for cast_result in &cast_results {
        wall_distances.push(calc_distance_between_points(&camera.pos, &cast_result));
    }

    let mut loop_count: usize = 0;
    for wall_distance in wall_distances {
        if !level.get_tile(&cast_results[loop_count]).transparent {
            let wall_height: f32 = (SCREEN_HEIGHT as f32 * 0.8) / wall_distance;
            let texture: &Texture = level.get_texture(&cast_results[loop_count]);
            for i in 0..texture.height - 1 {
                draw_rect_to_buffer_distanced(
                    dest,
                    &mut Rect {
                        x: SLICE_WIDTH * loop_count,
                        y: ((SCREEN_HEIGHT as f32 - wall_height) / 2.0) as usize
                            + ((wall_height / (texture.height as f32)) * i as f32) as usize,
                        width: SLICE_WIDTH + 1,
                        height: (wall_height / (texture.height as f32)) as usize + 1,
                        color: *texture.get_color(&Point {
                            x: ((cast_results[loop_count].x + cast_results[loop_count].y)
                                * (texture.width as f32))
                                % (texture.width as f32),
                            y: i as f32,
                        }),
                    },
                    wall_distance,
                );
            }
        }
        loop_count += 1;
    }
}
fn calc_distance_between_points(point1: &Point, point2: &Point) -> f32 {
    ((point1.x - point2.x).powf(2.0) + (point1.y - point2.y).powf(2.0)).powf(0.5)
}
fn cast_ray(pos: &Point, rotation: &Rotation, level: &Level) -> Point {
    const STEP: f32 = 0.01;

    let mut distance_travelled: f32 = 0.0;
    let mut point_to_check = Point::new(pos.x, pos.y);
    let mut has_hit: bool = false;
    while distance_travelled.abs() < RENDER_DISTANCE {
        distance_travelled += STEP;

        point_to_check = Point::new(
            pos.x + (distance_travelled * rotation.to_rad().cos()),
            pos.y + (distance_travelled * rotation.to_rad().sin()),
        );
        if !level.get_tile(&point_to_check).transparent {
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
fn draw_buffer_to_canvas(buffer: Vec<u8>, dest: &web_sys::CanvasRenderingContext2d) {
    dest.put_image_data(
        &web_sys::ImageData::new_with_u8_clamped_array(
            wasm_bindgen::Clamped { 0: &buffer },
            SCREEN_WIDTH as u32,
        )
        .unwrap(),
        0.0,
        0.0,
    );
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

    // let current_level: Level = Level::new(vec![
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //     ],
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //     ],
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //     ],
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //     ],
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Air),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //     ],
    //     vec![
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //         Tile::new(TileType::Stone),
    //     ],
    // ]);
    let current_level = Level::new(
        vec![
            vec![1, 1, 1, 1, 1],
            vec![1, 0, 0, 0, 1],
            vec![1, 0, 1, 0, 1],
            vec![1, 0, 0, 0, 1],
            vec![1, 1, 0, 1, 1],
        ],
        vec![Tile::new(0, false, true), Tile::new(1, true, false)],
        vec![
            Texture::new(
                vec![vec![0, 0], vec![0, 0]],
                vec![Color::new(255, 255, 255)],
            ),
            Texture::new(
                vec![
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0],
                    vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
                    vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
                ],
                vec![Color::new(205, 84, 75), Color::new(123, 46, 47)],
            ),
        ],
    );

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
                {
                    let mut buffer: Vec<u8> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

                    draw_background(&mut buffer);
                    draw_walls_to_buffer(&mut buffer, &PLAYER_CAMERA, &current_level);
                    draw_minimap(&mut buffer, &PLAYER_CAMERA, &current_level);

                    draw_buffer_to_canvas(buffer, &game_canvas);
                }
            } else {
                draw_not_running(&game_canvas);
            }
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}
