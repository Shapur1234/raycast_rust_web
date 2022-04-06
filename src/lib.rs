mod texture_consts;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

static mut SCREEN_WIDTH: usize = 0;
static mut SCREEN_HEIGHT: usize = 0;

static mut MOVEMENT_SPEED_MODIFIER: f32 = 0.05;
static mut GAME_RUNNING: bool = false;
static mut POINTER_SHOULD_BE_LOCKED: bool = false;
static mut PLAYER_CAMERA: Camera = Camera {
    pos: Point { x: 6.5, y: 7.5 },
    rotation: Rotation { degree: 0.0 },
    fov: 90,
    resolution_multiplier: 8,
    fish_eye_correction: true,
};

// --------------------------------------------------------------------------------

#[macro_use(c)]
extern crate cute;

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

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: Color,
}

impl Rect {
    fn new() -> Rect {
        unimplemented!();
    }
    fn fit_to_screen(&self, dest_width: usize, dest_height: usize) -> Rect {
        let mut rect_temp: Rect = *self;

        if rect_temp.x >= dest_width {
            rect_temp.x = dest_width - 1
        }
        if rect_temp.y >= dest_height {
            rect_temp.y = dest_height - 1
        }
        if rect_temp.x + rect_temp.width >= dest_width {
            rect_temp.width = dest_width - rect_temp.x
        }
        if rect_temp.y + rect_temp.height >= dest_height {
            rect_temp.height = dest_height - rect_temp.y
        }

        rect_temp
    }
}

// --------------------------------------------------------------------------------

enum Textures {
    Blank, 
    BrickWall,
    Richardo,
}

#[derive(Debug, Clone)]
struct Texture {
    width: usize,
    height: usize,
    layout: Vec<Color>,
}

impl Texture {
    fn new(id: Textures) -> Texture {
       match id {
        Textures::Blank => Texture {
            width: texture_consts::BLANK.0,
            height: texture_consts::BLANK.1,
            layout:  vec_u8_to_vec_color_with_trans(texture_consts::BLANK.2.to_vec()),
        },
        Textures::BrickWall => Texture {
            width: texture_consts::BRICK_WALL.0,
            height: texture_consts::BRICK_WALL.1,
            layout:  vec_u8_to_vec_color_with_trans(texture_consts::BRICK_WALL.2.to_vec()),
        },
        Textures::Richardo => Texture {
            width: texture_consts::RICHARDO.0,
            height: texture_consts::RICHARDO.1,
            layout:  vec_u8_to_vec_color(texture_consts::RICHARDO.2.to_vec()),
        },
       }
    }
    fn get_color(&self, point: &Point) -> &Color {
        if (point.x >= 0.0 && point.x < (self.width as f32)) && (point.y >= 0.0 && point.y < (self.height as f32)) {
            &self.layout[(self.width * (point.y as usize)) + (point.x as usize)]
        } else {
            &self.layout[0]
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
        Level {
            width: layout[0].len(),
            height: layout.len(),
            layout,
            all_tiles,
            all_textures,
        }
    }
    fn get_tile(&self, point: &Point) -> &Tile {
        if (point.x >= 0.0 && point.x < (self.width as f32)) && (point.y >= 0.0 && point.y < (self.height as f32)) {
            &self.all_tiles[self.layout[point.y as usize][point.x as usize] as usize]
        } else {
            &self.all_tiles[0]
        }
    }
    fn get_texture(&self, point: &Point) -> &Texture {
        &self.all_textures[self.get_tile(point).texture_index as usize]
    }
    fn is_in_level(&self, point: &Point) -> bool {
        if (point.x < 0.0 || point.x > self.width as f32) || (point.y < 0.0 || point.y > self.height as f32) {
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
    fn shade_distance(&self, distance: f32) -> Color {
        Color::new(
            ((self.r as f32 / (distance.powf(0.8))) as u8).clamp(self.r / 16, self.r),
            ((self.g as f32 / (distance.powf(0.8))) as u8).clamp(self.g / 16, self.g),
            ((self.b as f32 / (distance.powf(0.8))) as u8).clamp(self.b / 16, self.b),
        )
    }
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
            texture_index,
            solid,
            transparent,
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
    fov: u8,
    resolution_multiplier: u8,
    fish_eye_correction: bool,
}

impl Camera {
    fn new(pos: Point) -> Camera {
        Camera {
            pos: Point::new(pos.x, pos.y),
            rotation: Rotation::new(0.0),
            fov: 90,
            resolution_multiplier: 16,
            fish_eye_correction: true,
        }
    }
    fn get_angles_to_cast(&self) -> Vec<Rotation> {
        let mut output: Vec<Rotation> = Vec::new();
        for i in (self.rotation.degree - (self.fov as f32 / 2.0)) as i32
            ..(self.rotation.degree + (self.fov as f32 / 2.0)) as i32
        {
            for x in 0..self.resolution_multiplier {
                output.push(Rotation::new(
                    (i as f32) + ((1.0 / (self.resolution_multiplier as f32)) * (x as f32)),
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
            x_change += unsafe { MOVEMENT_SPEED_MODIFIER } * self.rotation.degree.to_radians().cos();
            y_change += unsafe { MOVEMENT_SPEED_MODIFIER } * self.rotation.degree.to_radians().sin();
        }
        if input.backward {
            x_change -= unsafe { MOVEMENT_SPEED_MODIFIER } * self.rotation.degree.to_radians().cos();
            y_change -= unsafe { MOVEMENT_SPEED_MODIFIER } * self.rotation.degree.to_radians().sin();
        }
        if input.right {
            x_change += unsafe { MOVEMENT_SPEED_MODIFIER } * (self.rotation.degree.to_radians() + 1.570796).cos();
            y_change += unsafe { MOVEMENT_SPEED_MODIFIER } * (self.rotation.degree.to_radians() + 1.570796).sin();
        }
        if input.left {
            x_change += unsafe { MOVEMENT_SPEED_MODIFIER } * (self.rotation.degree.to_radians() - 1.570796).cos();
            y_change += unsafe { MOVEMENT_SPEED_MODIFIER } * (self.rotation.degree.to_radians() - 1.570796).sin();
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

#[derive(Debug, Clone, Copy)]
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

struct FrameBuffer {
    buffer: Vec<u8>,
    width: usize,
    height: usize,
}

impl FrameBuffer {
    fn new(width: usize, height: usize) -> FrameBuffer {
        FrameBuffer {
            width,
            height,
            buffer: vec![0; width * height * 4],
        }
    }
    fn draw_pixel(&mut self, pos: Point, color: Color) {
        if pos.x >= 0.0 && pos.x <= (self.width as f32) && pos.y >= 0.0 && pos.y <= (self.height as f32) {
            let index: usize = (((pos.y as usize) * self.width) + (pos.x as usize)) * 4;

            self.buffer[index + 0] = color.r;
            self.buffer[index + 1] = color.g;
            self.buffer[index + 2] = color.b;
            self.buffer[index + 3] = 255;
        }
    }
    fn draw_rect(&mut self, rect: Rect) {
        let rect_temp = rect.fit_to_screen(self.width, self.height);

        for y in 0..rect_temp.height {
            for x in 0..rect_temp.width {
                let pos: usize = (((rect_temp.y + y) * self.width) + rect_temp.x + x) * 4;
                self.buffer[pos + 0] = rect_temp.color.r;
                self.buffer[pos + 1] = rect_temp.color.g;
                self.buffer[pos + 2] = rect_temp.color.b;
                self.buffer[pos + 3] = 255;
            }
        }
    }
    fn draw_line(&mut self, p0: Point, p1: Point, color: Color) {
        let (dx, dy) = ((p0.x - p1.x).abs(), -(p0.y - p1.y).abs());
        let (sx, sy) = (
            (if p0.x < p1.x { 1.0 } else { -1.0 }),
            (if p0.y < p1.y { 1.0 } else { -1.0 }),
        );
        let mut error = dx + dy;

        let mut point: Point = p0;
        for _ in 0..(((self.width.pow(2) * self.height.pow(2)) as f32).powf(0.5) as usize) {
            self.draw_pixel(point, color);

            if point.x == p1.x && point.y == p1.y {
                break;
            }
            if (error * 2.0) >= dy {
                if point.x == p1.x {
                    break;
                }
                error += dy;
                point.x += sx;
            }
            if (error * 2.0) <= dx {
                if point.y == p1.y {
                    break;
                }
                error += dx;
                point.y += sy;
            }
        }
    }
    fn draw_background(&mut self) {
        self.draw_rect(Rect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
            color: Color::new(0x4f, 0x4f, 0x4f),
        })
    }
    fn flip_to_canvas(&self, canvas: &web_sys::CanvasRenderingContext2d) {
        match canvas.put_image_data(
            &web_sys::ImageData::new_with_u8_clamped_array(
                wasm_bindgen::Clamped { 0: &self.buffer },
                self.width as u32,
            )
            .unwrap(),
            0.0,
            0.0,
        ) {
            Ok(_) => {}
            Err(e) => console_log!("Error drawing FrameBuffer to canvas: {:?}", e),
        }
    }
    fn draw_minimap(&mut self, camera: &Camera, level: &Level) {
        const TILE_SIZE: usize = 16;

        for y in 0..level.height {
            for x in 0..level.width {
                let tile_pos = &Point::new(x as f32, y as f32);
                let tile = level.get_tile(tile_pos);
                let texture = level.get_texture(tile_pos);
                self.draw_rect(Rect {
                    x: self.width - ((x + 2) * TILE_SIZE),
                    y: (y + 1) * TILE_SIZE,
                    width: TILE_SIZE,
                    height: TILE_SIZE,
                    color: if !tile.transparent {
                        texture.layout[0]
                    } else {
                        Color::new(255, 255, 255)
                    },
                })
            }
        }

        for angle in camera.get_angles_to_cast() {
            let cast_result: Point = cast_ray(&camera.pos, &angle, level).0;
            self.draw_line(
                Point::new(
                    (self.width - ((camera.pos.x + 1.0) * (TILE_SIZE as f32)) as usize) as f32,
                    (((camera.pos.y + 1.0) * (TILE_SIZE as f32)) as usize) as f32,
                ),
                Point::new(
                    (self.width - ((cast_result.x + 1.0) * (TILE_SIZE as f32)) as usize) as f32,
                    (((cast_result.y + 1.0) * (TILE_SIZE as f32)) as usize) as f32,
                ),
                Color::new(0, 255, 0),
            );
        }

        if level.is_in_level(&camera.pos) {
            self.draw_rect(Rect {
                x: self.width - ((camera.pos.x + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 4,
                y: ((camera.pos.y + 1.0) * (TILE_SIZE as f32)) as usize - TILE_SIZE / 4,
                width: TILE_SIZE / 2,
                height: TILE_SIZE / 2,
                color: Color::new(255, 0, 0),
            })
        }
    }
    fn draw_walls(&mut self, camera: &Camera, level: &Level) {
        let slice_width: f32 = (self.width as f32) / ((camera.fov as f32) * (camera.resolution_multiplier as f32));
        let mut cast_distances: Vec<f32> = vec![];
        let mut cast_points: Vec<Point> = vec![];

        for angle in camera.get_angles_to_cast() {
            let (cast_point, cast_distance) = cast_ray(&camera.pos, &angle, &level);
            cast_points.push(cast_point);
            cast_distances.push(
                cast_distance
                    * if camera.fish_eye_correction {
                        (angle.degree - camera.rotation.degree).to_radians().cos()
                    } else {
                        1.0
                    },
            );
        }

        let mut loop_count: usize = 0;
        for wall_distance in cast_distances {
            if !level.get_tile(&cast_points[loop_count]).transparent {
                let wall_height: f32 = (self.height as f32) / wall_distance;
                let texture: &Texture = level.get_texture(&cast_points[loop_count]);
                for i in 0..texture.height {
                    let vertical_slice_height: f32 = wall_height / (texture.height as f32);
                    self.draw_rect(Rect {
                        x: (slice_width * (loop_count as f32)) as usize,
                        y: (((self.height as f32 - wall_height) / 2.0)
                            + vertical_slice_height * (i as f32)
                            + if texture.height >= 8 {
                                vertical_slice_height / 2.0
                            } else {
                                0.0
                            }) as usize,
                        width: (slice_width + 1.0) as usize,
                        height: (wall_height / (texture.height as f32)) as usize + 1,
                        color: texture
                            .get_color(&Point {
                                x: ((cast_points[loop_count].x + cast_points[loop_count].y) * (texture.width as f32))
                                    % (texture.width as f32),
                                y: i as f32,
                            })
                            .shade_distance(wall_distance),
                    })
                }
            }
            loop_count += 1;
        }
    }
}

// --------------------------------------------------------------------------------

fn cast_ray(pos: &Point, rotation: &Rotation, level: &Level) -> (Point, f32) {
    let ray_dir: (f32, f32) = (rotation.degree.to_radians().cos(), rotation.degree.to_radians().sin());
    let mut map_pos: (i32, i32) = (pos.x as i32, pos.y as i32);
    let mut side_dist: (f32, f32) = (0.0, 0.0);
    let delta_dist: (f32, f32) = ((1.0 / ray_dir.0).abs(), (1.0 / ray_dir.1).abs());
    let mut step: (i32, i32) = (0, 0);
    let mut side: u8 = 0;

    if ray_dir.0 < 0.0 {
        step.0 = -1;
        side_dist.0 = (pos.x - map_pos.0 as f32) * delta_dist.0;
    } else {
        step.0 = 1;
        side_dist.0 = (((map_pos.0 + 1) as f32) - pos.x) * delta_dist.0;
    }

    if ray_dir.1 < 0.0 {
        step.1 = -1;
        side_dist.1 = (pos.y - map_pos.1 as f32) * delta_dist.1;
    } else {
        step.1 = 1;
        side_dist.1 = (((map_pos.1 + 1) as f32) - pos.y) * delta_dist.1;
    }

    for _ in 0..100 {
        if side_dist.0 < side_dist.1 {
            side_dist.0 += delta_dist.0;
            map_pos.0 += step.0;
            side = 0;
        } else {
            side_dist.1 += delta_dist.1;
            map_pos.1 += step.1;
            side = 1;
        }

        if !level
            .get_tile(&Point::new(map_pos.0 as f32, map_pos.1 as f32))
            .transparent
        {
            break;
        }
    }
    let distance: f32 = if side == 0 {
        side_dist.0 - delta_dist.0 + 0.0001
    } else {
        side_dist.1 - delta_dist.1 + 0.0001
    };
    (
        Point::new(pos.x + (ray_dir.0 * distance), pos.y + (ray_dir.1 * distance)),
        distance,
    )
}

fn vec_u8_to_vec_color_with_trans(pixels: Vec<u8>) -> Vec<Color> {
    c![Color::new(pixels[i * 4], pixels[i * 4 + 1], pixels[i * 4 + 2]), for i in 0..(pixels.len() / 4)]
}

fn vec_u8_to_vec_color(pixels: Vec<u8>) -> Vec<Color> {
    c![Color::new(pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]), for i in 0..(pixels.len() / 3)]
}


#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let game_canvas_html = document
        .get_element_by_id("game_canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();
    let game_canvas = game_canvas_html
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    let current_level = Level::new(
        vec![
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            vec![1, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 1],
            vec![1, 0, 0, 0, 2, 0, 1, 0, 2, 0, 0, 0, 1],
            vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1],
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1],
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1],
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1],
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1],
            vec![1, 0, 0, 0, 2, 0, 1, 0, 2, 0, 0, 0, 1],
            vec![1, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 1],
            vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ],
        vec![
            Tile::new(0, false, true),
            Tile::new(1, true, false),
            Tile::new(2, true, false),
        ],
        vec![
            Texture::new(Textures::Blank),
            Texture::new(Textures::BrickWall),
            Texture::new(Textures::Richardo),
        ],
    );
    // console_log!("{:?}", Texture::new(String::from("BrickWall")));
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

                if pressed_key == 97 {
                    PLAYER_CAMERA.resolution_multiplier -= 1;
                    PLAYER_CAMERA.resolution_multiplier = PLAYER_CAMERA.resolution_multiplier.clamp(1, 32);
                    console_log!(
                        "PLAYER_CAMERA.resolution_multiplier changed to: {:?}",
                        PLAYER_CAMERA.resolution_multiplier
                    );
                } else if pressed_key == 98 {
                    PLAYER_CAMERA.resolution_multiplier += 1;
                    PLAYER_CAMERA.resolution_multiplier = PLAYER_CAMERA.resolution_multiplier.clamp(1, 32);
                    console_log!(
                        "PLAYER_CAMERA.resolution_multiplier changed to: {:?}",
                        PLAYER_CAMERA.resolution_multiplier
                    );
                }
                if pressed_key == 99 {
                    PLAYER_CAMERA.fish_eye_correction = !PLAYER_CAMERA.fish_eye_correction;
                }

                if pressed_key == 100 {
                    PLAYER_CAMERA.fov -= 1;
                    PLAYER_CAMERA.fov = PLAYER_CAMERA.fov.clamp(4, 180);
                    console_log!("FOV changed to: {:?}", PLAYER_CAMERA.fov);
                } else if pressed_key == 101 {
                    PLAYER_CAMERA.fov += 1;
                    PLAYER_CAMERA.fov = PLAYER_CAMERA.fov.clamp(4, 180);
                    console_log!("FOV changed to: {:?}", PLAYER_CAMERA.fov);
                }
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Mouse input
    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| unsafe {
            if GAME_RUNNING {
                PLAYER_CAMERA.rotation.degree += ((event.movement_x() * 10) as f32) * MOVEMENT_SPEED_MODIFIER;
            }
        }) as Box<dyn FnMut(_)>);
        game_canvas_html.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Mouse click
    {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            if !unsafe { GAME_RUNNING } {
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

                unsafe {
                    GAME_RUNNING = true;
                    POINTER_SHOULD_BE_LOCKED = true;
                }
            }
        }) as Box<dyn FnMut(_)>);
        game_canvas_html.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Pointerlock exit
    {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| unsafe {
            if POINTER_SHOULD_BE_LOCKED {
                POINTER_SHOULD_BE_LOCKED = false;
            } else {
                GAME_RUNNING = false;
            }
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback("pointerlockchange", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Pointerlock error
    {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| unsafe {
            POINTER_SHOULD_BE_LOCKED = false;
            GAME_RUNNING = false;
        }) as Box<dyn FnMut(_)>);
        document.add_event_listener_with_callback("pointerlockerror", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    // Game loop
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        {
            // TODO: Make into event
            {
                unsafe {
                    SCREEN_WIDTH = window.inner_width().unwrap().as_f64().unwrap() as usize;
                    SCREEN_HEIGHT = window.inner_height().unwrap().as_f64().unwrap() as usize;
                }
                game_canvas_html.set_width(unsafe { SCREEN_WIDTH } as u32);
                game_canvas_html.set_height(unsafe { SCREEN_HEIGHT } as u32);
            }
            let mut frame_buffer: FrameBuffer = FrameBuffer::new(unsafe { SCREEN_WIDTH }, unsafe { SCREEN_HEIGHT });

            frame_buffer.draw_background();
            frame_buffer.draw_walls(unsafe { &PLAYER_CAMERA }, &current_level);
            frame_buffer.draw_minimap(unsafe { &PLAYER_CAMERA }, &current_level);

            frame_buffer.flip_to_canvas(&game_canvas);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}
