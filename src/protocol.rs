/// Commands that can be sent to a DGT board
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Request clock data
    RequestClock = 0x41,
    /// Request complete board state
    RequestBoard = 0x42,
    /// Enable update mode
    EnableUpdate = 0x43,
    /// Request board update
    RequestUpdate = 0x44,
    /// Request serial number
    RequestSerialNumber = 0x45,
    /// Request bus address
    RequestBusAddress = 0x46,
    /// Request trademark
    RequestTrademark = 0x47,
    /// Request version
    RequestVersion = 0x4d,
    /// Request "nice" update mode
    RequestNiceUpdate = 0x4b,
    /// Request EE moves
    RequestEEMoves = 0x49,
    /// Reset board
    Reset = 0x40,
}

impl Command {
    /// Convert the command to a byte for sending over serial
    pub fn as_byte(self) -> [u8; 1] {
        [self as u8]
    }

    /// Try to convert a byte into a Command
    pub fn try_from_byte(byte: u8) -> Option<Self> {
        use Command::*;
        match byte {
            0x41 => Some(RequestClock),
            0x42 => Some(RequestBoard),
            0x43 => Some(EnableUpdate),
            0x44 => Some(RequestUpdate),
            0x45 => Some(RequestSerialNumber),
            0x46 => Some(RequestBusAddress),
            0x47 => Some(RequestTrademark),
            0x4d => Some(RequestVersion),
            0x4b => Some(RequestNiceUpdate),
            0x49 => Some(RequestEEMoves),
            0x40 => Some(Reset),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remaining {
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl Remaining {
    fn new(hours: u8, minutes: u8, seconds: u8) -> Self {
        Remaining {
            hours,
            minutes,
            seconds,
        }
    }

    fn from_bcd(bcd: [u8; 3]) -> Self {
        let hours = bcd[0];
        let minutes = bcd[1];
        let seconds = bcd[2];
        Remaining {
            hours: 10 * (hours >> 4) + (hours & 0x0f),
            minutes: 10 * (minutes >> 4) + (minutes & 0x0f),
            seconds: 10 * (seconds >> 4) + (seconds & 0x0f),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockStatus {
    NoCock,
    WhitesTurn,
    BlacksTurn,
}

impl ClockStatus {
    fn from_byte(byte: u8) -> Self {
        if byte & 0x01 != 0 {
            ClockStatus::NoCock
        } else if byte & 0x08 != 0 {
            ClockStatus::BlacksTurn
        } else {
            ClockStatus::WhitesTurn
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChessBoard {
    pub board: [RawPiece; 64],
}

impl ChessBoard {
    fn new(raw: &[u8; 64]) -> Option<Self> {
        let mut board = Vec::new();
        for s in raw.iter() {
            if let Some(piece) = RawPiece::try_from_byte(*s) {
                board.push(piece);
            } else {
                return None;
            }
        }
        Some(ChessBoard {
            board: board.try_into().unwrap(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChessMove {
    pub grid: u8,
    pub piece: RawPiece,
}

impl ChessMove {
    fn new(grid: u8, piece: RawPiece) -> Self {
        ChessMove { grid, piece }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceColor {
    None,
    White,
    Black,
}

/// Raw piece representation as sent by DGT board
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RawPiece {
    #[default]
    Empty = 0x00,
    WhitePawn = 0x01,
    WhiteRook = 0x02,
    WhiteKnight = 0x03,
    WhiteBishop = 0x04,
    WhiteKing = 0x05,
    WhiteQueen = 0x06,
    BlackPawn = 0x07,
    BlackRook = 0x08,
    BlackKnight = 0x09,
    BlackBishop = 0x0a,
    BlackKing = 0x0b,
    BlackQueen = 0x0c,
}

impl RawPiece {
    /// Convert a byte into a RawPiece, returning None for invalid values
    pub fn try_from_byte(byte: u8) -> Option<Self> {
        use RawPiece::*;
        match byte {
            0x00 => Some(Empty),
            0x01 => Some(WhitePawn),
            0x02 => Some(WhiteRook),
            0x03 => Some(WhiteKnight),
            0x04 => Some(WhiteBishop),
            0x05 => Some(WhiteKing),
            0x06 => Some(WhiteQueen),
            0x07 => Some(BlackPawn),
            0x08 => Some(BlackRook),
            0x09 => Some(BlackKnight),
            0x0a => Some(BlackBishop),
            0x0b => Some(BlackKing),
            0x0c => Some(BlackQueen),
            _ => None,
        }
    }

    /// Convert the piece to a FEN character representation
    pub fn to_char(self) -> char {
        use RawPiece::*;
        match self {
            Empty => ' ',
            WhitePawn => 'P',
            WhiteRook => 'R',
            WhiteKnight => 'N',
            WhiteBishop => 'B',
            WhiteKing => 'K',
            WhiteQueen => 'Q',
            BlackPawn => 'p',
            BlackRook => 'r',
            BlackKnight => 'n',
            BlackBishop => 'b',
            BlackKing => 'k',
            BlackQueen => 'q',
        }
    }

    /// Get the color of the piece
    fn get_colour(&self) -> PieceColor {
        match self {
            RawPiece::Empty => PieceColor::None,
            RawPiece::WhitePawn
            | RawPiece::WhiteRook
            | RawPiece::WhiteKnight
            | RawPiece::WhiteBishop
            | RawPiece::WhiteKing
            | RawPiece::WhiteQueen => PieceColor::White,
            RawPiece::BlackPawn
            | RawPiece::BlackRook
            | RawPiece::BlackKnight
            | RawPiece::BlackBishop
            | RawPiece::BlackKing
            | RawPiece::BlackQueen => PieceColor::Black,
        }
    }

    /// Check if two pieces are the same color
    fn is_same_colour(&self, other: &RawPiece) -> bool {
        *self != RawPiece::Empty && self.get_colour() == other.get_colour()
    }
}

/// Message types that can be received from a DGT board
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    BoardDump = 0x06,
    BWTime = 0x0d,
    FieldUpdate = 0x0e,
    EEMoves = 0x0f,
    BusAddress = 0x10,
    SerialNumber = 0x11,
    Trademark = 0x12,
    Version = 0x13,
}

impl MessageType {
    pub fn try_from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x06 => Some(MessageType::BoardDump),
            0x0d => Some(MessageType::BWTime),
            0x0e => Some(MessageType::FieldUpdate),
            0x0f => Some(MessageType::EEMoves),
            0x10 => Some(MessageType::BusAddress),
            0x11 => Some(MessageType::SerialNumber),
            0x12 => Some(MessageType::Trademark),
            0x13 => Some(MessageType::Version),
            _ => None,
        }
    }
}

/// Decoded responses from the DGT board
#[derive(Debug)]
pub enum Response {
    /// Complete board state
    BoardDump(ChessBoard),
    /// Clock data for both players and active color
    BWTime {
        white_time: Remaining,
        black_time: Remaining,
        status: ClockStatus,
    },
    /// Single piece movement
    FieldUpdate(ChessMove),
    /// Board serial number
    SerialNumber(String),
    /// Bus address information
    BusAddress(String),
    /// Board trademark information
    Trademark(String),
    /// Board version information
    Version(String),
}

impl Response {
    /// Attempt to parse a raw message into a decoded response
    pub fn try_from_raw(message_type: MessageType, data: &[u8]) -> Result<Self, ParseError> {
        match message_type {
            MessageType::BoardDump => {
                if data.len() == 64 {
                    match ChessBoard::new(data.try_into().unwrap()) {
                        Some(board) => Ok(Response::BoardDump(board)),
                        None => Err(ParseError::InvalidPiece),
                    }
                } else {
                    Err(ParseError::invalid_length(message_type, 64, data.len()))
                }
            }
            MessageType::BWTime => {
                if data.len() == 7 {
                    let white_time = Remaining::from_bcd(data[..3].try_into().unwrap());
                    let black_time = Remaining::from_bcd(data[3..6].try_into().unwrap());
                    let status = ClockStatus::from_byte(data[6]);
                    Ok(Response::BWTime {
                        white_time,
                        black_time,
                        status,
                    })
                } else {
                    Err(ParseError::invalid_length(message_type, 7, data.len()))
                }
            }
            MessageType::FieldUpdate => {
                if data.len() == 2 {
                    let grid = data[0];
                    if grid < 64 {
                        if let Some(piece) = RawPiece::try_from_byte(data[1]) {
                            Ok(Response::FieldUpdate(ChessMove::new(grid, piece)))
                        } else {
                            Err(ParseError::InvalidPiece)
                        }
                    } else {
                        Err(ParseError::InvalidMove)
                    }
                } else {
                    Err(ParseError::invalid_length(message_type, 2, data.len()))
                }
            }
            MessageType::SerialNumber => Ok(Response::SerialNumber(
                String::from_utf8_lossy(data).into_owned(),
            )),
            MessageType::EEMoves => {
                todo!("Implement EEMoves parsing")
            }
            MessageType::BusAddress => Ok(Response::BusAddress(
                String::from_utf8_lossy(data).into_owned(),
            )),
            MessageType::Trademark => Ok(Response::Trademark(
                String::from_utf8_lossy(data).into_owned(),
            )),
            MessageType::Version => {
                if data.len() == 2 {
                    let version = format!("{}.{}", data[0], data[1]);
                    Ok(Response::Version(version))
                } else {
                    Err(ParseError::invalid_length(message_type, 2, data.len()))
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    InvalidLength {
        message_type: MessageType,
        expected: usize,
        actual: usize,
    },
    InvalidPiece,
    InvalidMove,
}

impl ParseError {
    fn invalid_length(message_type: MessageType, expected: usize, actual: usize) -> Self {
        ParseError::InvalidLength {
            message_type,
            expected,
            actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let data = &[1u8, 2u8];
        let response = Response::try_from_raw(MessageType::Version, data).unwrap();
        assert!(matches!(response, Response::Version(v) if v == "1.2"));
    }

    #[test]
    fn test_command_roundtrip() {
        let cmd = Command::RequestBoard;
        let byte = cmd.as_byte();
        let cmd2 = Command::try_from_byte(byte[0]).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_invalid_command() {
        assert_eq!(Command::try_from_byte(0x00), None);
    }

    #[test]
    fn test_piece_conversion() {
        // Test valid pieces
        assert_eq!(RawPiece::try_from_byte(0x00), Some(RawPiece::Empty));
        assert_eq!(RawPiece::try_from_byte(0x01), Some(RawPiece::WhitePawn));
        assert_eq!(RawPiece::try_from_byte(0x0c), Some(RawPiece::BlackQueen));

        // Test invalid piece
        assert_eq!(RawPiece::try_from_byte(0x0d), None);
    }

    #[test]
    fn test_piece_to_char() {
        assert_eq!(RawPiece::Empty.to_char(), ' ');
        assert_eq!(RawPiece::WhiteKing.to_char(), 'K');
        assert_eq!(RawPiece::BlackPawn.to_char(), 'p');
    }
}
