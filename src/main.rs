#[macro_use]
extern crate lazy_static;
extern crate rustbox;

use std::collections::HashSet;
use std::default::Default;
use std::env;
use std::ffi::OsString;
use std::fs::{OpenOptions};
use std::io::{Read, Write};

use rustbox::Key;
use rustbox::{Color, RustBox};

// assumed as a reasonable? line length
const LINE_VECTOR_CAPACITY: usize = 100;


lazy_static! {
    static ref RUST_KEYWORDS: HashSet<&'static str> = [
        "abstract", "alignof", "as", "become", "box",
        "break", "const", "continue", "crate", "do",
        "else", "enum", "extern", "false", "final",
        "fn", "for", "if", "impl", "in", "let", "loop",
        "macro", "match", "mod", "move", "mut", "offsetof",
        "override", "priv", "proc", "pub", "pure", "ref",
        "return", "Self", "self", "sizeof", "static",
        "struct", "super", "trait", "true", "type",
        "typeof", "unsafe", "unsized", "use", "virtual",
        "where", "while", "yield"
    ].iter().cloned().collect();
    static ref RUST_SYMBOLS: HashSet<&'static str> = [
        ":", ";", "(", ")", "[", "]", "{", "}", "=", "<", ">", "->", "\"",
    ].iter().cloned().collect();
}

pub enum BufferChanges {
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
}

pub struct Display {
    rustbox: RustBox,
    width: usize,
    height: usize,
    vertical_offset: usize,
}

impl Display {
    fn new() -> Display {
        let rustbox = match RustBox::init(Default::default()) {
            Result::Ok(v) => v,
            Result::Err(e) => panic!("{}", e),
        };
        let width = rustbox.width();
        let height = rustbox.height();
        Display {
            rustbox: rustbox,
            width: width,
            height: height,
            vertical_offset: 0
        }
    }

    fn clear_line(&self, line_number: usize) {
        let blank_line: String = (0..self.width).into_iter().map(|_| " ").collect();
        self.rustbox.print(0, line_number,
                           rustbox::RB_NORMAL,
                           Color::White,
                           Color::Black,
                           &blank_line);
    }

    fn render_cursor(&self, cursor: &Cursor, vertical_offset: usize) {
        self.rustbox.set_cursor(cursor.x as isize,
                                (cursor.y - vertical_offset) as isize);
    }

    fn render_word(&self, word: &str, offset: usize, line_number: usize, color: Color) -> usize {
        let word = if word.len() == 0 {
            " ".to_string()
        } else if offset != 0 {
            [" ", word].concat()
        } else {
            word.to_string()
        };
        self.rustbox.print(offset, line_number,
                           rustbox::RB_NORMAL,
                           color,
                           Color::Black,
                           &word);
        word.len()
    }

    fn render_line(&self, line: &str, line_number: usize) {
        self.clear_line(line_number);
        let mut offset = 0;
        let mut is_comment = false;
        let mut is_string = false;
        let mut is_char = false;

        for word in line.split(" ") {
            if is_comment || word == "//" || word.starts_with("//") {
                is_comment = true;
                offset += self.render_word(word, offset, line_number, Color::Blue);
            } else if RUST_KEYWORDS.contains(&word) {
                offset += self.render_word(word, offset, line_number, Color::Green);
            } else if word.len() == 0 {
                offset += self.render_word(word, offset, line_number, Color::Green);
            } else {
                // go char by char
                if offset != 0 {
                    self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                    Color::Default, Color::Black,
                                    " ");
                    offset += 1;
                };
                for character in word.chars() {
                    if character == '"' && !is_string && !is_char {  // open string
                        is_string = true;
                        // paint string
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Yellow, Color::Black,
                                           &character.to_string());
                    } else if is_string && character == '"' { // close string
                        is_string = false;
                        // paint string
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Yellow, Color::Black,
                                           &character.to_string());
                    } else if is_string || is_char {
                        // paint string
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Yellow, Color::Black,
                                           &character.to_string());
                    } else if character == '\'' && !is_char {  // open char
                        is_char = true;
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                            Color::Yellow, Color::Black,
                                            &character.to_string());
                    } else if is_char && character == '\'' {  // close char
                        is_char = false;
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Yellow, Color::Black,
                                           &character.to_string());
                    } else if RUST_SYMBOLS.contains(&(character.to_string()[..])) {
                        // paint symbol
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Red, Color::Black,
                                           &character.to_string());
                    } else {
                        // normal
                        self.rustbox.print(offset, line_number, rustbox::RB_NORMAL,
                                           Color::Default, Color::Black,
                                           &character.to_string());
                    }
                    offset += 1;
                }
            }
        }
    }

    fn render_buffer_changes(&self, buffer: &Buffer, changes: BufferChanges) {
        match changes {
            BufferChanges::Buffer          => self.render_buffer(buffer),
            BufferChanges::Lines(lines)    => {
                for line_number in lines {
                    self.render_line(
                        &buffer.get_line(line_number),
                        line_number - self.vertical_offset
                    );
                }
            }
            BufferChanges::Char(_) => {unimplemented!()},
            BufferChanges::None            => {},
        };
    }

    fn render_buffer(&self, buffer: &Buffer) {
        self.rustbox.clear();
        for i in self.vertical_offset..(self.vertical_offset + self.height) {
            self.render_line(&buffer.get_line(i), i - self.vertical_offset);
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

    fn write_char(&mut self, cursor: &Cursor, character: char) -> BufferChanges {
        let &Cursor{x, y} = cursor;
        self.fill_lines(y);

        let mut line = self.data.get_mut(y).unwrap();
        while x > line.len() { line.push(' '); }

        if line.len() > x {
            line.insert(x, character);
        } else {
            line.push(character);
        }
        BufferChanges::Lines(vec![y])
    }

    fn newline(&mut self, cursor: &Cursor) -> BufferChanges {
        let &Cursor{x, y} = cursor;
        // make sure we have enough lines
        self.fill_lines(y);
        self.insert_line(y+1);

        if let Some(rest) = self.get_line_data_from_offset(y, x) {
            self.truncate_line(y, x);
            let mut new_line = self.data.get_mut(y+1).unwrap();
            new_line.extend(rest);
            // we could optimize here if we have little following lines
            BufferChanges::Buffer
        } else {
            BufferChanges::None
        }
    }

    fn get_line_length(&self, line_number: usize) -> usize {
        if line_number >= self.data.len() {
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

    fn backspace(&mut self, cursor: &Cursor) -> BufferChanges {
        let &Cursor{x, y} = cursor;
        let mut result = BufferChanges::None;

        if let Some(line) = self.data.get_mut(y) {
            if line.len() + 1 > x && x > 0 {
                line.remove(x-1);
                result = BufferChanges::Buffer;
            }
        }

        // if we want to delete back from the first position of a line,
        // slurp the next line.
        if x == 0 && y > 0 {
            self.slurp_next_line(y-1);
            self.remove_line(y);
            result = BufferChanges::Buffer;
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
    let new_line = ['\n'];
    let file = OpenOptions::new().write(true).truncate(true).create(true).open(&filename);
    let string = buffer.data.iter().flat_map(|line| {
        line.iter().chain(new_line.iter())
    }).cloned().collect::<String>();

    if let Ok(mut file) = file {
        let _ = file.write(string.as_bytes());
    } else {
       panic!("Couldn't open file for writing.");
    }
}

fn read_file_as_string(filename: &OsString) -> Option<String> {
    let file = OpenOptions::new().read(true).open(filename);
    if let Ok(mut file) = file {
        let mut file_contents = String::new();
        if let Ok(_) = file.read_to_string(&mut file_contents) {
            return Some(file_contents);
        }
    }
    None
}

fn get_filename_or_exit() -> OsString {
    let cli_arguments = env::args_os();
    if cli_arguments.len() < 2 {
        println!("Please provide a filename to read or create.");
        std::process::exit(1);
    }
    cli_arguments.skip(1).next().unwrap()
}

pub fn get_next_cursor(current_cursor: &Cursor, buffer: &Buffer, direction: Key) -> Cursor {
    let &Cursor{x, y} = current_cursor;

    let valid_movement: bool = match (x, y, direction) {
        // We can only go up if we're somewhere other than the first line
        (_, y, Key::Up) if y > 0 => true,
        // We can only go down if there's more lines in the buffer "below"
        (_, y, Key::Down) if y + 1 < buffer.count_lines() => true,
        // Valid left movements are when in the middle of a line or at the
        // beginning of a line other than the first one
        (x, y, Key::Left) if x > 0 || y > 0 => true,
        // We can only go right if we haven't reach the end of the last line
        (x, y, Key::Right) if x < buffer.get_line_length(y) || y < buffer.count_lines() => true,
        _ => false
    };

    if !valid_movement { return Cursor::new(x, y); }

    match direction {
        Key::Left  => {
            // if we're at the beginning of a line, jump back to the previous
            // one if possible
            if y > 0 && x == 0 {
                Cursor::new(buffer.get_line_length(y-1), y-1)
            } else {
                Cursor::new(x-1, y)
            }
        },
        Key::Right => {
            // if we're at the end of a line, jump to the beginning of the next
            // one
            if y + 1 < buffer.count_lines() && x == buffer.get_line_length(y) {
                Cursor::new(0, y+1)
            } else {
                Cursor::new(x+1, y)
            }
        },
        Key::Up    => {
            // if previous line's length is lower than x, go to its EOL
            if buffer.get_line_length(y-1) < x {
                Cursor::new(buffer.get_line_length(y-1), y-1)
            } else {
                Cursor::new(x, y-1)
            }
        }
        Key::Down  => {
            // if next line's length is lower than x, go to its EOL
            if buffer.get_line_length(y+1) < x {
                Cursor::new(buffer.get_line_length(y+1), y+1)
            } else {
                Cursor::new(x, y+1)
            }
        },
        _          => unreachable!()
    }
}


fn apply_command(key: Key, buffer: &mut Buffer, cursor: &Cursor) -> (BufferChanges, Cursor) {
    match key {
        Key::Char(character) => {
            (buffer.write_char(cursor, character), Cursor::new(cursor.x + 1, cursor.y))
        },
        Key::Enter           => {
            let buffer_changes = buffer.newline(cursor);
            let new_cursor = Cursor::new(0, cursor.y + 1);
            (buffer_changes, new_cursor)
        },
        Key::Backspace       => {
            let previous_line_length = if cursor.y > 0 {
                buffer.get_line_length(cursor.y-1)
            } else {
                0
            };

            let changes = buffer.backspace(&cursor);

            let new_cursor = if cursor.x > 0 {
                Cursor::new(cursor.x - 1, cursor.y)
            } else if cursor.y > 0 {
                Cursor::new(previous_line_length, cursor.y - 1)
            } else {
                Cursor::new(cursor.x, cursor.y)
            };

            (changes, new_cursor)
        }
        _ => {(BufferChanges::None, Cursor::new(cursor.x, cursor.y))}
    }
}


fn main() {
    let filename = get_filename_or_exit();
    let mut display = Display::new();
    let mut cursor = Cursor::new(0, 0);
    let mut buffer = if let Some(file_contents) = read_file_as_string(&filename) {
        Buffer::from_string(&file_contents)
    } else {
        Buffer::new()
    };

    display.render_buffer(&buffer);
    display.render_cursor(&cursor, display.vertical_offset);
    display.flush();

    loop {
        let mut buffer_changes = BufferChanges::None;
        match display.rustbox.poll_event(false) {
            Ok(rustbox::Event::KeyEvent(key)) => {
                match key {
                    Key::Ctrl('q')       => { break; },
                    Key::Ctrl('s')       => { save_to_file(&filename, &buffer); },
                    Key::Right           => { cursor = get_next_cursor(&cursor, &buffer, key); },
                    Key::Left            => { cursor = get_next_cursor(&cursor, &buffer, key); },
                    Key::Down            => { cursor = get_next_cursor(&cursor, &buffer, key); },
                    Key::Up              => { cursor = get_next_cursor(&cursor, &buffer, key); },
                    _ => {
                        let result = apply_command(key, &mut buffer, &cursor);
                        buffer_changes = result.0;
                        cursor = result.1;
                    },
                }
            },
            _ => { }
        };

        if cursor.y >= display.vertical_offset + display.height {
            // scroll down
            display.vertical_offset += 1;
            buffer_changes = BufferChanges::Buffer;
        }
        else if cursor.y < display.vertical_offset {
            // scroll up
            display.vertical_offset -= 1;
            buffer_changes = BufferChanges::Buffer;
        }

        // only render buffer changes if there's been any
        display.render_buffer_changes(&buffer, buffer_changes);
        display.render_cursor(&cursor, display.vertical_offset);
        display.flush();
    }
}


#[cfg(test)]
mod tests {

    use super::*;
    use rustbox::Key;

    fn enums_are_equal(changes: BufferChanges, expected: BufferChanges) -> bool {
        match (changes, expected) {
            (BufferChanges::None, BufferChanges::None) => true,
            (BufferChanges::Buffer, BufferChanges::Buffer) => true,
            (BufferChanges::Char(pair_0), BufferChanges::Char(pair_1)) => pair_0 == pair_1,
            (BufferChanges::Lines(vec_0), BufferChanges::Lines(vec_1)) => vec_0 == vec_1,
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
        // let expected_changes_0 = BufferChanges::Lines(vec![0]);
        let expected_changes_0 = BufferChanges::Buffer;
        let cursor = Cursor::new(9, 0);
        let changes_0 = buffer_0.backspace(&cursor);
        assert_eq!(true, enums_are_equal(changes_0, expected_changes_0));
        assert_eq!(buffer_0.count_lines(), 1);
        assert_eq!(buffer_0.get_line(0), "I'm a typo.");

        // cursor at (0,0), should do nothing
        let mut buffer_1 = Buffer::from_string("I'm still a tipo");
        let expected_changes_1 = BufferChanges::None;
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

    #[test]
    fn test_cursor_movements_off_limits() {
        let empty_buffer = Buffer::new();
        let original_cursor = Cursor::new(0, 0);

        // try to go down, nothing happens
        let next_cursor = get_next_cursor(&original_cursor, &empty_buffer, Key::Down);
        assert_eq!(next_cursor.x, original_cursor.x);
        assert_eq!(next_cursor.y, original_cursor.y);

        // try to go up, nothing happens
        let next_cursor = get_next_cursor(&original_cursor, &empty_buffer, Key::Up);
        assert_eq!(next_cursor.x, original_cursor.x);
        assert_eq!(next_cursor.y, original_cursor.y);

        // try to go left, nothing happens
        let next_cursor = get_next_cursor(&original_cursor, &empty_buffer, Key::Left);
        assert_eq!(next_cursor.x, original_cursor.x);
        assert_eq!(next_cursor.y, original_cursor.y);

        // try to go right, nothing happens
        let next_cursor = get_next_cursor(&original_cursor, &empty_buffer, Key::Right);
        assert_eq!(next_cursor.x, original_cursor.x);
        assert_eq!(next_cursor.y, original_cursor.y);
    }

    #[test]
    fn test_cursor_movements_off_limits_exceptions() {
        let line_0 = "I'm Line 0";
        let line_1 = "And here's Line 1.";
        let line_2 = "Line 1 here";
        let buffer = Buffer::from_string(&[line_0, line_1, line_2].join("\n"));

        // Moving left at the beginning of a line should make the cursor
        // jump to the last character of previous line.
        let original_cursor = Cursor::new(0, 1);
        let expected_cursor = Cursor::new(line_0.len(), 0);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Left);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // Moving right at the end of a line should make the cursor
        // jump to the first character of the next line.
        let original_cursor = Cursor::new(line_1.len(), 1);
        let expected_cursor = Cursor::new(0, 2);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Right);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // Moving down when the next line is shorter, should move the cursor to EOL
        let original_cursor = Cursor::new(line_1.len(), 1);
        let expected_cursor = Cursor::new(line_2.len(), 2);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Down);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // Moving up when the previous line is shorter, should move the cursor to EOL
        let original_cursor = Cursor::new(line_1.len(), 1);
        let expected_cursor = Cursor::new(line_0.len(), 0);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Up);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

    }

    #[test]
    fn test_cursor_movements_happy_path() {
        let buffer = Buffer::from_string("I'm Line 0\nLine 1 here\nAnd here's Line 2.");

        // move from the middle of the line to the left
        let original_cursor = Cursor::new(3, 0);
        let expected_cursor = Cursor::new(original_cursor.x - 1, original_cursor.y);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Left);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // move from the middle of the line to the right
        let original_cursor = Cursor::new(3, 0);
        let expected_cursor = Cursor::new(original_cursor.x + 1, original_cursor.y);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Right);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // move up
        let original_cursor = Cursor::new(3, 1);
        let expected_cursor = Cursor::new(original_cursor.x, original_cursor.y-1);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Up);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);

        // move down
        let original_cursor = Cursor::new(3, 1);
        let expected_cursor = Cursor::new(original_cursor.x, original_cursor.y+1);
        let next_cursor = get_next_cursor(&original_cursor, &buffer, Key::Down);
        assert_eq!(next_cursor.x, expected_cursor.x);
        assert_eq!(next_cursor.y, expected_cursor.y);
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
