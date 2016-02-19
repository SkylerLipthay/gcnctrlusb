extern crate ansi_term;
extern crate gcnctrlusb;

use std::cell::Cell;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    let controllers = Arc::new(Mutex::new(Cell::new(Default::default())));

    let controllers_write = controllers.clone();
    thread::spawn(move || {
        loop {
            let mut scanner = gcnctrlusb::Scanner::new().unwrap();
            let mut adapter = if let Some(adapter) = scanner.find_adapter().unwrap() {
                adapter
            } else {
                continue;
            };
            let mut listener = adapter.listen().unwrap();

            while let Ok(controllers) = listener.read() {
                controllers_write.lock().unwrap().set(controllers);
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    loop {
        // Clear screen:
        io::stdout().write(b"\x1b[2J\x1b[;H").unwrap();
        for controller in &controllers.lock().unwrap().get() {
            draw_controller(controller);
        }
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_millis(100));
    }
}

// ___       __           __        __                __
//  |  |  | |__) |\ |    |__)  /\  /  ` |__/    |\ | /  \ |  |
//  |  \__/ |  \ | \|    |__) /~~\ \__, |  \    | \| \__/ |/\|
//
// My apologies for this code. I warned you :)

fn draw_controller(controller: &Option<gcnctrlusb::Controller>) {
    use ansi_term::Colour::Fixed;

    let controller = match *controller {
        Some(ref controller) => controller,
        None => return,
    };

    fn trigger_colors(a: u8, d: bool) -> [u8; 6] {
        if d {
            return [15; 6];
        }
        let mut result = [239; 6];
        let mut i = 0;
        while i < 6 && i < a / 35 {
            result[i as usize] = 247;
            i += 1;
        }
        result
    }

    fn stick_colors(x: u8, y: u8, back: u8, fore: u8) -> [[u8; 6]; 6] {
        let mut result = [[back; 6]; 6];
        let x = (x / 52) as usize;
        let y = (y / 52) as usize;
        result[x][y] = fore;
        result[x + 1][y] = fore;
        result[x][y + 1] = fore;
        result[x + 1][y + 1] = fore;
        result
    }

    fn dpad_colors(l: bool, r: bool, u: bool, d: bool) -> [[u8; 4]; 4] {
        let mut result = [[239; 4]; 4];
        if l {
            result[0][1] = 247;
            result[0][2] = 247;
        }
        if r {
            result[3][1] = 247;
            result[3][2] = 247;
        }
        if d {
            result[1][0] = 247;
            result[2][0] = 247;
        }
        if u {
            result[1][3] = 247;
            result[2][3] = 247;
        }
        result
    }

    let l = trigger_colors(controller.l_analog, controller.l);
    let sl = stick_colors(controller.stick_x, controller.stick_y, 239, 247);
    let d = dpad_colors(controller.left, controller.right, controller.up, controller.down);
    let s = if controller.start { 247 } else { 239 };
    let sc = stick_colors(controller.c_stick_x, controller.c_stick_y, 136, 220);
    let b = if controller.b { 196 } else { 52 };
    let a = if controller.a { 48 } else { 23 };
    let x = if controller.x { 247 } else { 239 };
    let y = if controller.y { 247 } else { 239 };
    let r = trigger_colors(controller.r_analog, controller.r);
    let z = if controller.z { 105 } else { 62 };

    fn p<'a>(bottom: u8, top: u8) -> ansi_term::ANSIString<'a> {
        Fixed(bottom).on(Fixed(top)).paint("▄")
    }

    fn ps(c: &[[u8; 6]; 6], r: u8) -> String {
        let r = (r * 2) as usize;
        if r == 4 {
            format!("{}{}{}{}{}{}",
                    p(c[0][r], 0), p(c[1][r], c[1][r + 1]), p(c[2][r], c[2][r + 1]),
                    p(c[3][r], c[3][r + 1]), p(c[4][r], c[4][r + 1]), p(c[5][r], 0))
        } else if r == 2 {
            format!("{}{}{}{}{}{}",
                    p(c[0][r], c[0][r + 1]), p(c[1][r], c[1][r + 1]), p(c[2][r], c[2][r + 1]),
                    p(c[3][r], c[3][r + 1]), p(c[4][r], c[4][r + 1]), p(c[5][r], c[5][r + 1]))
        } else {
            format!("{}{}{}{}{}{}",
                    p(0, c[0][r + 1]), p(c[1][r], c[1][r + 1]), p(c[2][r], c[2][r + 1]),
                    p(c[3][r], c[3][r + 1]), p(c[4][r], c[4][r + 1]), p(0, c[5][r + 1]))
        }
    }

    fn pd(c: &[[u8; 4]; 4], r: u8) -> String {
        if r == 0 {
            format!("{}{}{}{}",
                    p(0, c[0][1]), p(c[1][0], c[1][1]), p(c[2][0], c[2][1]), p(0, c[3][1]))
        } else {
            format!("{}{}{}{}",
                    p(c[0][2], 0), p(c[1][2], c[1][3]), p(c[2][2], c[2][3]), p(c[3][2], 0))
        }
    }

    println!("                                            ");
    println!(" {}  {}            {}    {}{}          {} {} ", p(l[4], l[5]), ps(&sl, 2), ps(&sc, 2), p(y, y), p(y, y), p(z, z), p(r[4], r[5]));
    println!(" {}  {}  {}  {}{}  {}       {}{}{}{} {}{}  {} {} ", p(l[2], l[3]), ps(&sl, 1), pd(&d, 1), p(s, s), p(s, s), ps(&sc, 1), p(a, a), p(a, a), p(a, a), p(a, a), p(x, x), p(x, x), p(z, z), p(r[2], r[3]));
    println!(" {}  {}  {}      {}  {}{}   {}{}{}{}     {} {} ", p(l[0], l[1]), ps(&sl, 0), pd(&d, 0), ps(&sc, 0), p(b, b), p(b, b), p(a, a), p(a, a), p(a, a), p(a, a), p(z, z), p(r[0], r[1]));
    println!("                                            ");
}
