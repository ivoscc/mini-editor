extern crate rustbox;

use std::env;
use std::default::Default;
use std::fs::{OpenOptions};
use std::io::{Read, Write};
use std::ffi::OsString;

use rustbox::{Color, RustBox};
use rustbox::Key;

// assumed as a reasonable? line length
const LINE_VECTOR_CAPACITY: usize = 100;

pub enum Changes {
    Char((usize, usize)),
    Lines(Vec<usize>),
    Buffer,
    None
}

pub struct Cursor {
    x: usize,
    y: usize,
}

impl Cursor {
    fn new(x: usize, y: usize) -> Cursor {
        Cursor {x: x, y: y}
    }

    fn next(&mut self) {
        self.x += 1;
    }

    fn back(&mut self) {
        if self.x > 0 {
            self.x -= 1;
        } else if self.y > 0 {
            self.y -= 1;
        }
    }

    fn newline(&mut self) {
        self.x = 0;
        self.y += 1;
    }

    fn jump_to(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }

}

pub struct Display {
    rustbox: RustBox,
    width: usize,
    height: usize,
}

impl Display {
    fn new() -> Display {
        let rustbox = match RustBox::init(Default::default()) {
            Result::Ok(v) => v,
            Result::Err(e) => panic!("{}", e),
        };
        let width = rustbox.width();
        let height = rustbox.height();
        Display { rustbox: rustbox, width: width, height: height }
    }

    fn clear_line(&self, line_number: usize) {
        let blank_line: String = (0..self.width).into_iter().map(|x| " ").collect();
        self.rustbox.print(0, line_number,
                           rustbox::RB_NORMAL,
                           Color::White,
                           Color::Black,
                           &blank_line);
    }

    fn render_cursor(&self, cursor: &Cursor) {
        self.rustbox.set_cursor(cursor.x as isize, cursor.y as isize);
    }

    fn render_line(&self, line: &str, line_number: usize) {
        self.clear_line(line_number);
        self.rustbox.print(0, line_number,
                           rustbox::RB_NORMAL,
                           Color::White,
                           Color::Black,
                           line);
    }

    fn render_buffer_changes(&self, buffer: &Buffer, changes: Changes) {
        match changes {
            Changes::Buffer          => self.render_buffer(buffer),
            Changes::Lines(lines)    => {
                for line_number in lines {

                    self.render_line(
                        &buffer.get_line(line_number),
                        line_number
                    );
                }
            }
            Changes::Char(character) => {},  // not implemented
            Changes::None            => {},
        };
    }

    fn render_buffer(&self, buffer: &Buffer) {
        self.rustbox.clear();
        for i in 0..buffer.count_lines() {
            self.render_line(&buffer.get_line(i), i);
        }
    }

    fn flush(&self) {
        self.rustbox.present();
    }
}

pub struct Buffer {
    data: Vec<Vec<char>>,
}

impl Buffer {
    fn new() -> Buffer {
        Buffer {data: Vec::new()}
    }

    fn from_string(string: &str) -> Buffer {
        let data = string.lines().map(|line| {
            line.chars().collect::<Vec<char>>()
        }).collect::<Vec<Vec<char>>>();
        Buffer {data: data}
    }

    fn write_char(&mut self, cursor: &Cursor, character: char) -> Changes {
        let &Cursor{x, y} = cursor;
        self.fill_lines(y);

        let mut line = self.data.get_mut(y).unwrap();
        while x > line.len() { line.push(' '); }

        if line.len() > x {
            line.insert(x, character);
        } else {
            line.push(character);
        }
        Changes::Lines(vec![y])
    }

    fn newline(&mut self, cursor: &Cursor) -> Changes {
        let &Cursor{x, y} = cursor;
        // make sure we have enough lines
        self.fill_lines(y);
        self.insert_line(y+1);

        if let Some(rest) = self.get_line_data_from_offset(y, x) {
            self.truncate_line(y, x);
            let mut new_line = self.data.get_mut(y+1).unwrap();
            new_line.extend(rest);
            // we could optimize here if we have little following lines
            Changes::Buffer
        } else {
            Changes::None
        }
    }

    fn remove_trailing_empty_lines(&mut self,) {
        let number_of_lines = self.count_lines();
        let mut empty_trailing_lines_counter = 0;
        for line_number in (0..number_of_lines).rev() {
            if !self.data[line_number].is_empty() {
                break;
            }
            empty_trailing_lines_counter += 1;
        }
        self.data.truncate(number_of_lines-empty_trailing_lines_counter);
    }

    fn get_eol_position(&self, line_number: usize) -> usize {
        if line_number < 0 || line_number >= self.data.len() {
            return 0;
        }

        if let Some(line) = self.data.get(line_number) {
            line.len()
        } else {
            0
        }
    }

    fn remove_line(&mut self, line_number: usize) {
        if self.count_lines() > line_number {
            self.data.remove(line_number);
        }
    }

    fn slurp_next_line(&mut self, line_number: usize) {
        let next_line_content = self.get_line(line_number+1);
        let mut first_line = &mut self.data[line_number];
        first_line.extend(next_line_content.chars().into_iter());
    }

    fn backspace(&mut self, cursor: &Cursor) -> Changes {
        let &Cursor{x, y} = cursor;
        let mut result = Changes::None;

        if let Some(line) = self.data.get_mut(y) {
            if line.len() + 1 > x && x > 0 {
                line.remove(x-1);
                result = Changes::Buffer;
            }
        }

        // if we want to delete back from the first position of a line,
        // slurp the next line.
        if x == 0 && y > 0 {
            self.slurp_next_line(y-1);
            self.remove_line(y);
            result = Changes::Buffer;
        }

        result
    }

    fn count_lines(&self) -> usize {
        self.data.len()
    }

    fn get_line(&self, line_number: usize) -> String {
        if let Some(line) = self.data.get(line_number) {
            line.iter().cloned().collect()
        } else {
            "".to_string()
        }
    }

    fn insert_line(&mut self, line_number: usize) {
        self.data.insert(line_number, Vec::with_capacity(LINE_VECTOR_CAPACITY));
    }

    fn fill_lines(&mut self, line_number: usize) {
        while line_number + 1 > self.count_lines() || self.count_lines() == 0 {
            self.data.push(Vec::with_capacity(LINE_VECTOR_CAPACITY));
        }
    }

    fn get_line_data_from_offset(&mut self, line_number: usize, offset: usize) -> Option<Vec<char>> {
        let mut result = None;
        if let Some(line) = self.data.get(line_number) {
            if line.len() >= offset {
                let (_, rest) = line.split_at(offset);
                result = Some(rest.iter().cloned().collect());
            }
        }
        result
    }

    fn truncate_line(&mut self, line_number: usize, offset: usize) {
        let mut original = self.data.get_mut(line_number).unwrap();
        original.truncate(offset);
    }
}

fn save_to_file(filename: &OsString, buffer: &Buffer) {

    let file = OpenOptions::new().write(true).truncate(true).create(true).open(&filename);
    let string = buffer.data.iter().flat_map(|line| line.into_iter()).cloned().collect::<String>();

    if let Ok(mut file) = file {
        file.write(string.as_bytes());
    } else {
       panic!("Couldn't open file for writing.");
    }

}

fn main() {

    let cli_arguments = env::args_os();
    if cli_arguments.len() < 2 {
        println!("Please provide a filename to read or create.");
        std::process::exit(1);
    }

    let filename = cli_arguments.skip(1).next().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .open(&filename);

    let mut buffer = if let Ok(mut file) = file {
        let mut file_contents = String::new();
        if let Ok(size) = file.read_to_string(&mut file_contents) {
            Buffer::from_string(&file_contents)
        } else {
            Buffer::new()
        }
    } else {
        Buffer::new()
    };

    let mut cursor = Cursor::new(0, 0);
    let display = Display::new();

    display.render_buffer(&buffer);
    display.render_cursor(&cursor);
    display.flush();

    loop {
        match display.rustbox.poll_event(false) {
            Ok(rustbox::Event::KeyEvent(key)) => {
                match key {
                    Key::Ctrl('q')       => { break; },
                    Key::Ctrl('s')       => {
                        save_to_file(&filename, &buffer);
                    },
                    Key::Right           => {
                        if cursor.x + 1 < buffer.get_eol_position(cursor.y) + 1 {
                            cursor.x += 1;
                            display.render_cursor(&cursor);
                        }
                    },
                    Key::Left            => {
                        if cursor.x > 0 {
                            cursor.x -= 1;
                            display.render_cursor(&cursor);
                        }
                    },
                    Key::Down            => {
                        if cursor.y + 1 < buffer.count_lines() {
                            let next_line_length = buffer.get_eol_position(cursor.y + 1);
                            cursor.y += 1;
                            if cursor.x > next_line_length {
                                cursor.x = next_line_length;
                            }
                            display.render_cursor(&cursor);
                        }
                    },
                    Key::Up              => {
                        if cursor.y > 0 {
                            let previous_line = cursor.y-1;
                            let previous_line_length = buffer.get_eol_position(previous_line);
                            if cursor.x > previous_line_length {
                                cursor.jump_to(previous_line_length, previous_line);
                            } else {
                                cursor.y -= 1;
                            }

                            display.render_cursor(&cursor);
                        }
                    }
                    Key::Char(character) => {
                        let changes = buffer.write_char(&cursor, character);
                        display.render_buffer_changes(&buffer, changes);
                        cursor.next();
                        display.render_cursor(&cursor);
                    },
                    Key::Backspace       => {
                        let mut previous_line_length = 0;
                        if cursor.y > 0 {
                             previous_line_length = buffer.get_eol_position(cursor.y-1);
                        }
                        let changes = buffer.backspace(&cursor);
                        if cursor.x == 0 && cursor.y > 0 {
                            cursor.y -= 1;
                            cursor.x = previous_line_length;
                        } else if cursor.x > 0 {
                            cursor.x -= 1;
                        }
                        display.render_buffer_changes(&buffer, changes);
                        display.render_cursor(&cursor);
                    },
                    Key::Enter           => {
                        let changes = buffer.newline(&cursor);
                        display.render_buffer_changes(&buffer, changes);
                        cursor.newline();
                        display.render_cursor(&cursor);
                    },
                    _ => {}
                }
            },
            Err(e) => panic!("{}", e),
            _ => { }
        };
        display.flush();
    }
}


#[cfg(test)]
mod tests {

    use super::*;

    fn enums_are_equal(changes: Changes, expected: Changes) -> bool {
        match (changes, expected) {
            (Changes::None, Changes::None) => true,
            (Changes::Buffer, Changes::Buffer) => true,
            (Changes::Char(pair_0), Changes::Char(pair_1)) => pair_0 == pair_1,
            (Changes::Lines(vec_0), Changes::Lines(vec_1)) => vec_0 == vec_1,
            _ => false,
        }
    }

    #[test]
    fn test_initialize_buffer_from_string() {
        // initialize Buffer from a string
        let expected_string_0 = "Hello there";
        let buffer_0 = Buffer::from_string(expected_string_0);
        assert_eq!(buffer_0.count_lines(), 1);
        assert_eq!(buffer_0.get_line(0), expected_string_0);

        // intiialize Buffer from a multiline string
        let expected_string_1 = "Hi there.\nI'm a string.\nMe too!";
        let buffer_1 = Buffer::from_string(expected_string_1);
        assert_eq!(buffer_1.count_lines(), 3);
        assert_eq!(buffer_1.get_line(0), "Hi there.");
        assert_eq!(buffer_1.get_line(1), "I'm a string.");
        assert_eq!(buffer_1.get_line(2), "Me too!");
    }

    #[test]
    fn test_add_character() {
        let buffer = Buffer::new();

        // sanity check
        assert_eq!(buffer.count_lines(), 0);

        // we need a mutable buffer for writing
        let mut buffer = buffer;

        // write at position (0, 0)
        let cursor = Cursor::new(0, 0);
        buffer.write_char(&cursor, 'h');
        assert_eq!(buffer.count_lines(), 1);
        assert_eq!(buffer.get_line(0), "h");

        // write at position (0, 1)
        let cursor = Cursor::new(1, 0);
        buffer.write_char(&cursor, 'i');
        assert_eq!(buffer.count_lines(), 1);
        assert_eq!(buffer.get_line(0), "hi");

        // write at position (10, 10)
        let cursor = Cursor::new(10, 10);
        buffer.write_char(&cursor, 'x');
        assert_eq!(buffer.count_lines(), 11);
        assert_eq!(buffer.get_line(10), "          x");
    }

    #[test]
    fn test_write_character_mid_line() {
        let mut buffer = Buffer::new();
        let cursor = Cursor::new(5, 0);

        buffer.write_char(&cursor, 'i');
        assert_eq!(buffer.count_lines(), 1);
        assert_eq!(buffer.get_line(0), "     i");

        buffer.write_char(&cursor, 'h');
        assert_eq!(buffer.count_lines(), 1);
        assert_eq!(buffer.get_line(0), "     hi");
    }

    #[test]
    fn test_insert_newline() {
        // empty_line
        let mut buffer = Buffer::new();
        let cursor = Cursor::new(0, 0);
        buffer.newline(&cursor);
        assert_eq!(buffer.count_lines(), 2);
        assert_eq!(buffer.get_line(0), "");
        assert_eq!(buffer.get_line(1), "");

        // line with data
        let mut buffer = Buffer::from_string("Hello world.");
        let cursor = Cursor::new(5, 0);
        buffer.newline(&cursor);
        assert_eq!(buffer.count_lines(), 2);
        assert_eq!(buffer.get_line(0), "Hello");
        assert_eq!(buffer.get_line(1), " world.");
    }

    #[test]
    fn test_delete_one_character() {
        let mut buffer_0 = Buffer::from_string("I'm a typpo.");
        // let expected_changes_0 = Changes::Lines(vec![0]);
        let expected_changes_0 = Changes::Buffer;
        let cursor = Cursor::new(9, 0);
        let changes_0 = buffer_0.backspace(&cursor);
        assert_eq!(true, enums_are_equal(changes_0, expected_changes_0));
        assert_eq!(buffer_0.count_lines(), 1);
        assert_eq!(buffer_0.get_line(0), "I'm a typo.");

        // cursor at (0,0), should do nothing
        let mut buffer_1 = Buffer::from_string("I'm still a tipo");
        let expected_changes_1 = Changes::None;
        let cursor = Cursor::new(0, 0);
        let changes_1 = buffer_1.backspace(&cursor);
        assert_eq!(true, enums_are_equal(changes_1, expected_changes_1));
        assert_eq!(buffer_1.count_lines(), 1);
        assert_eq!(buffer_1.get_line(0), "I'm still a tipo");
    }

    #[test]
    fn test_delete_from_first_position_in_line() {
        let mut buffer = Buffer::from_string("Line 1\nA\nLine 3");
        assert_eq!(buffer.count_lines(), 3);
        let cursor = Cursor::new(1, 1);
        buffer.backspace(&cursor);
        assert_eq!(buffer.count_lines(), 3);
        let cursor = Cursor::new(0, 1);
        buffer.backspace(&cursor);
        assert_eq!(buffer.count_lines(), 2);

        assert_eq!(buffer.get_line(0), "Line 1");
        assert_eq!(buffer.get_line(1), "Line 3");
    }

    #[test]
    fn test_backspace_line_content_go_up() {
        let mut buffer = Buffer::from_string("Something\nElse");
        let cursor = Cursor::new(0, 1);
        assert_eq!(buffer.count_lines(), 2);
        buffer.backspace(&cursor);
        assert_eq!(buffer.count_lines(), 1);
        assert_eq!(buffer.get_line(0), "SomethingElse");
    }

    // #[test]
    // fn test_backspace_at_0_0_should_do_nothing(){
    // }

    // #[test]
    // fn test_cursor_goes_previous_eol_after_slurping_line() {
    // }

    // #[test]
    // fn moving_up_or_down_to_a_smaller_lines_moves_cursor_to_eol() {
    // }

    // #[test]
    // fn prevent_moving_cursor_beyond_eol() {
    // }

}
