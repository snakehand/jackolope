use crate::protocol::*;
use chess::{
    Board, BoardBuilder, BoardStatus, CastleRights, Color, File, MoveGen, Piece, Square,
    ALL_COLORS, ALL_SQUARES,
};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub type FingerPrint = [u32; 8];

pub struct Capture {
    piece: RawPiece,
    grid: u8,
}

pub struct Move {
    piece: RawPiece,
    from: u8,
    to: u8,
}

pub enum DetectedMove {
    ShortCastle,
    LongCastle,
    PawnCapture(Capture),
    Promotion(Move, RawPiece),
    PromotionCapture(Move, Capture, RawPiece),
    SimpleMove(Move),
    SimpleCapture(Move, Capture),
}

fn detect_move(moves: &[ChessMove]) -> Option<DetectedMove> {
    let mut added = HashMap::new();
    let mut removed = HashSet::new();
    for mv in moves {
        if mv.piece == RawPiece::Empty {
            if let Entry::Occupied(e) = added.entry(mv.grid) {
                e.remove();
            }
            removed.insert(mv.grid);
        } else {
            if removed.contains(&mv.grid) {
                removed.remove(&mv.grid);
            }
            added.insert(mv.grid, mv.piece);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartPosition {
    None,
    Normal,
    Mirror,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameBoard {
    board: ChessBoard,
    start: StartPosition,
}

impl GameBoard {
    pub fn new(board: ChessBoard) -> GameBoard {
        let start = StartPosition::None;
        let mut game = GameBoard { board, start };
        game.start = game.is_starting_position();
        game
    }

    pub fn apply_move(&mut self, mv: ChessMove) {
        self.board.board[mv.grid as usize] = mv.piece;
        for i in 0..8 {
            let row: Vec<char> = self.board.board[i * 8..(i + 1) * 8]
                .iter()
                .map(|&x| x.to_char())
                .collect();
            println!("{:?}", row);
        }
    }

    pub fn is_starting_position(&mut self) -> StartPosition {
        if self.board.board[16..48]
            .iter()
            .any(|p| *p != RawPiece::Empty)
        {
            return StartPosition::None;
        }
        if self.board.board[8..16]
            .iter()
            .all(|p| *p == RawPiece::WhitePawn)
            && self.board.board[48..56]
                .iter()
                .all(|p| *p == RawPiece::BlackPawn)
            && self.board.board[0..8]
                == [
                    RawPiece::WhiteRook,
                    RawPiece::WhiteKnight,
                    RawPiece::WhiteBishop,
                    RawPiece::WhiteKing,
                    RawPiece::WhiteQueen,
                    RawPiece::WhiteBishop,
                    RawPiece::WhiteKnight,
                    RawPiece::WhiteRook,
                ]
            && self.board.board[56..64]
                == [
                    RawPiece::BlackRook,
                    RawPiece::BlackKnight,
                    RawPiece::BlackBishop,
                    RawPiece::BlackKing,
                    RawPiece::BlackQueen,
                    RawPiece::BlackBishop,
                    RawPiece::BlackKnight,
                    RawPiece::BlackRook,
                ]
        {
            self.start = StartPosition::Normal;
            return self.start;
        }
        if self.board.board[8..16]
            .iter()
            .all(|p| *p == RawPiece::BlackPawn)
            && self.board.board[48..56]
                .iter()
                .all(|p| *p == RawPiece::WhitePawn)
            && self.board.board[0..8]
                == [
                    RawPiece::BlackRook,
                    RawPiece::BlackKnight,
                    RawPiece::BlackBishop,
                    RawPiece::BlackQueen,
                    RawPiece::BlackKing,
                    RawPiece::BlackBishop,
                    RawPiece::BlackKnight,
                    RawPiece::BlackRook,
                ]
            && self.board.board[56..64]
                == [
                    RawPiece::WhiteRook,
                    RawPiece::WhiteKnight,
                    RawPiece::WhiteBishop,
                    RawPiece::WhiteQueen,
                    RawPiece::WhiteKing,
                    RawPiece::WhiteBishop,
                    RawPiece::WhiteKnight,
                    RawPiece::WhiteRook,
                ]
        {
            self.start = StartPosition::Mirror;
            return self.start;
        }

        StartPosition::None
    }

    /// Map a DGT grid index to a chess square index (a1 = 0, h1 = 7, a8 = 56).
    /// In the normal orientation grid 0 is h1, in the mirrored orientation grid 0 is a8.
    /// If no starting position has been seen yet, the normal orientation is assumed.
    fn square_of(&self, grid: usize) -> usize {
        let (row, col) = (grid / 8, grid % 8);
        match self.start {
            StartPosition::Mirror => (7 - row) * 8 + col,
            _ => row * 8 + (7 - col),
        }
    }

    /// Fingerprint of the board in canonical orientation, independent of how the
    /// board was set up, so it can be compared with positions from the chess crate.
    pub fn fingerprint(&self) -> FingerPrint {
        let mut res = [0; 8];
        for (grid, piece) in self.board.board.iter().enumerate() {
            set_piece_at(&mut res, self.square_of(grid), *piece);
        }
        res
    }
}

/// Fingerprint layout: one u32 per rank (rank 1 first), one nibble per file (a = lowest).
fn set_piece_at(fp: &mut FingerPrint, square: usize, piece: RawPiece) {
    let shift = 4 * (square % 8);
    fp[square / 8] = (fp[square / 8] & !(0xF << shift)) | ((piece as u32) << shift);
}

fn piece_at(fp: &FingerPrint, square: usize) -> RawPiece {
    let nibble = (fp[square / 8] >> (4 * (square % 8))) & 0xF;
    RawPiece::try_from_byte(nibble as u8).unwrap_or(RawPiece::Empty)
}

fn raw_from_chess(piece: Piece, color: Color) -> RawPiece {
    match (color, piece) {
        (Color::White, Piece::Pawn) => RawPiece::WhitePawn,
        (Color::White, Piece::Rook) => RawPiece::WhiteRook,
        (Color::White, Piece::Knight) => RawPiece::WhiteKnight,
        (Color::White, Piece::Bishop) => RawPiece::WhiteBishop,
        (Color::White, Piece::King) => RawPiece::WhiteKing,
        (Color::White, Piece::Queen) => RawPiece::WhiteQueen,
        (Color::Black, Piece::Pawn) => RawPiece::BlackPawn,
        (Color::Black, Piece::Rook) => RawPiece::BlackRook,
        (Color::Black, Piece::Knight) => RawPiece::BlackKnight,
        (Color::Black, Piece::Bishop) => RawPiece::BlackBishop,
        (Color::Black, Piece::King) => RawPiece::BlackKing,
        (Color::Black, Piece::Queen) => RawPiece::BlackQueen,
    }
}

fn chess_from_raw(piece: RawPiece) -> Option<(Piece, Color)> {
    match piece {
        RawPiece::Empty => None,
        RawPiece::WhitePawn => Some((Piece::Pawn, Color::White)),
        RawPiece::WhiteRook => Some((Piece::Rook, Color::White)),
        RawPiece::WhiteKnight => Some((Piece::Knight, Color::White)),
        RawPiece::WhiteBishop => Some((Piece::Bishop, Color::White)),
        RawPiece::WhiteKing => Some((Piece::King, Color::White)),
        RawPiece::WhiteQueen => Some((Piece::Queen, Color::White)),
        RawPiece::BlackPawn => Some((Piece::Pawn, Color::Black)),
        RawPiece::BlackRook => Some((Piece::Rook, Color::Black)),
        RawPiece::BlackKnight => Some((Piece::Knight, Color::Black)),
        RawPiece::BlackBishop => Some((Piece::Bishop, Color::Black)),
        RawPiece::BlackKing => Some((Piece::King, Color::Black)),
        RawPiece::BlackQueen => Some((Piece::Queen, Color::Black)),
    }
}

/// Fingerprint of a chess crate position, comparable with `GameBoard::fingerprint`.
pub fn board_fingerprint(board: &Board) -> FingerPrint {
    let mut res = [0; 8];
    for square in ALL_SQUARES {
        if let (Some(piece), Some(color)) = (board.piece_on(square), board.color_on(square)) {
            set_piece_at(&mut res, square.to_index(), raw_from_chess(piece, color));
        }
    }
    res
}

/// Build a position from a recorded board state. Castling rights are carried over from
/// the previous game, but only kept where king and rook are still on their home squares.
fn board_from_fingerprint(
    fp: &FingerPrint,
    side_to_move: Color,
    castling: [CastleRights; 2],
) -> Option<Board> {
    // Board::try_from looks up the kings before its sanity check, so make sure they exist.
    for king in [RawPiece::WhiteKing, RawPiece::BlackKing] {
        if (0..64).filter(|&sq| piece_at(fp, sq) == king).count() != 1 {
            return None;
        }
    }
    let mut builder = BoardBuilder::new();
    for square in ALL_SQUARES {
        let index = square.to_index();
        if let Some((piece, color)) = chess_from_raw(piece_at(fp, index)) {
            // The move generator can't deal with pawns on the back ranks.
            if piece == Piece::Pawn && !(8..56).contains(&index) {
                return None;
            }
            builder.piece(square, piece, color);
        }
    }
    builder.side_to_move(side_to_move);
    for color in ALL_COLORS {
        let home = if color == Color::White { 0 } else { 56 };
        let rook = raw_from_chess(Piece::Rook, color);
        let king_home = piece_at(fp, home + 4) == raw_from_chess(Piece::King, color);
        let rights = castling[color.to_index()];
        let kingside = king_home && rights.has_kingside() && piece_at(fp, home + 7) == rook;
        let queenside = king_home && rights.has_queenside() && piece_at(fp, home) == rook;
        builder.castle_rights(
            color,
            CastleRights::from_index(kingside as usize | (queenside as usize) << 1),
        );
    }
    Board::try_from(&builder).ok()
}

/// A legal half-move together with the position it leads to.
#[derive(Debug, Clone, Copy)]
pub struct Ply {
    pub mv: chess::ChessMove,
    pub board: Board,
    fp: FingerPrint,
}

impl Ply {
    fn new(mv: chess::ChessMove, board: Board) -> Self {
        let fp = board_fingerprint(&board);
        Self { mv, board, fp }
    }
}

/// The legal move from `board` that results in the recorded board state `fp`, if any.
fn find_child(board: &Board, fp: &FingerPrint) -> Option<Ply> {
    MoveGen::new_legal(board)
        .map(|mv| Ply::new(mv, board.make_move_new(mv)))
        .find(|ply| ply.fp == *fp)
}

/// Outcome of feeding a recorded board state to a `Partition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// The board shows a position already in the line (a piece put back, a take-back in progress).
    Known,
    /// The line was extended or corrected.
    Moved,
    /// The board state can't be explained, e.g. a piece is lifted, or an illegal move was made.
    Unexplained,
}

/// A legally connected sequence of moves from a starting position.
#[derive(Debug, Clone)]
pub struct Partition {
    start: Board,
    start_fp: FingerPrint,
    line: Vec<Ply>,
    /// Board updates since the line last matched the board.
    idle: usize,
    /// Half-moves inferred without their position having been recorded.
    skipped: usize,
    /// Half-moves played in the game before `start` (0 = white's first move).
    first_ply: usize,
}

impl Partition {
    fn new(start: Board) -> Self {
        let start_fp = board_fingerprint(&start);
        let line = Vec::new();
        Self {
            start,
            start_fp,
            line,
            idle: 0,
            skipped: 0,
            first_ply: 0,
        }
    }

    /// Set the half-move number of the starting position, so that it continues from a
    /// game that ended after `previous` half-moves.
    fn continue_from(&mut self, previous: usize) {
        let white_to_move = self.start.side_to_move() == Color::White;
        self.first_ply = if previous.is_multiple_of(2) == white_to_move {
            previous
        } else {
            previous + 1
        };
    }

    /// Number of half-moves in the game after the last move of this partition.
    fn last_ply(&self) -> usize {
        self.first_ply + self.line.len()
    }

    fn is_standard_start(&self) -> bool {
        self.first_ply == 0 && self.start == Board::default()
    }

    /// FEN of the starting position, with the move number filled in.
    fn fen(&self) -> String {
        let fen = self.start.to_string();
        let fullmove = (self.first_ply / 2 + 1).to_string();
        let mut fields: Vec<&str> = fen.split_whitespace().collect();
        if let Some(last) = fields.last_mut() {
            *last = &fullmove;
        }
        fields.join(" ")
    }

    /// Movetext in standard algebraic notation, wrapped to PGN export line length.
    fn movetext(&self, comment: Option<&str>, result: &str) -> String {
        let mut tokens: Vec<String> = Vec::new();
        if let Some(comment) = comment {
            tokens.push(format!("{{{}}}", comment));
        }
        let mut board = self.start;
        for (i, ply) in self.line.iter().enumerate() {
            let n = self.first_ply + i;
            if n.is_multiple_of(2) {
                tokens.push(format!("{}.", n / 2 + 1));
            } else if i == 0 {
                tokens.push(format!("{}...", n / 2 + 1));
            }
            tokens.push(san(&board, ply.mv));
            board = ply.board;
        }
        tokens.push(result.to_string());

        let mut text = String::new();
        let mut width = 0;
        for token in tokens.iter().flat_map(|t| t.split_whitespace()) {
            if width > 0 && width + 1 + token.len() > PGN_LINE_WIDTH {
                text.push('\n');
                width = 0;
            } else if width > 0 {
                text.push(' ');
                width += 1;
            }
            text.push_str(token);
            width += token.len();
        }
        text
    }

    pub fn moves(&self) -> impl Iterator<Item = chess::ChessMove> + '_ {
        self.line.iter().map(|ply| ply.mv)
    }

    pub fn tip(&self) -> &Board {
        self.board_at(self.line.len())
    }

    /// Position after `k` half-moves.
    fn board_at(&self, k: usize) -> &Board {
        if k == 0 {
            &self.start
        } else {
            &self.line[k - 1].board
        }
    }

    fn fp_at(&self, k: usize) -> &FingerPrint {
        if k == 0 {
            &self.start_fp
        } else {
            &self.line[k - 1].fp
        }
    }

    /// Greedily match a recorded board state against the legal continuations of the line.
    fn advance(&mut self, fp: &FingerPrint) -> Step {
        let step = self.match_fingerprint(fp);
        self.idle = if step == Step::Unexplained {
            self.idle + 1
        } else {
            0
        };
        step
    }

    fn match_fingerprint(&mut self, fp: &FingerPrint) -> Step {
        let n = self.line.len();
        // Walk back from the current position: a move from an earlier position means the
        // later moves were taken back, or the same piece was moved again before the clock.
        for k in (n - n.min(MAX_TAKEBACK)..=n).rev() {
            if self.fp_at(k) == fp {
                return Step::Known;
            }
            if let Some(ply) = find_child(self.board_at(k), fp) {
                self.line.truncate(k);
                self.line.push(ply);
                return Step::Moved;
            }
        }
        // A position may never have been recorded cleanly, so also allow two half-moves.
        let tip = *self.tip();
        for mv in MoveGen::new_legal(&tip) {
            let child = tip.make_move_new(mv);
            if let Some(ply) = find_child(&child, fp) {
                self.line.push(Ply::new(mv, child));
                self.line.push(ply);
                self.skipped += 1;
                return Step::Moved;
            }
        }
        Step::Unexplained
    }
}

impl fmt::Display for Partition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]", self.fen())?;
        for (i, mv) in self.moves().enumerate() {
            let ply = self.first_ply + i;
            if ply.is_multiple_of(2) {
                write!(f, " {}. {}", ply / 2 + 1, mv)?;
            } else if i == 0 {
                write!(f, " {}... {}", ply / 2 + 1, mv)?;
            } else {
                write!(f, " {}", mv)?;
            }
        }
        Ok(())
    }
}

fn piece_letter(piece: Piece) -> Option<char> {
    match piece {
        Piece::Pawn => None,
        Piece::Knight => Some('N'),
        Piece::Bishop => Some('B'),
        Piece::Rook => Some('R'),
        Piece::Queen => Some('Q'),
        Piece::King => Some('K'),
    }
}

/// Standard algebraic notation of a legal move, e.g. "Nbd2", "exd5", "e8=Q+", "O-O".
fn san(board: &Board, mv: chess::ChessMove) -> String {
    let (source, dest) = (mv.get_source(), mv.get_dest());
    let piece = board.piece_on(source).unwrap_or(Piece::Pawn);
    let changes_file = source.get_file() != dest.get_file();
    let mut san = String::new();
    let file_distance = source
        .get_file()
        .to_index()
        .abs_diff(dest.get_file().to_index());
    if piece == Piece::King && file_distance == 2 {
        san.push_str(if dest.get_file() == File::G {
            "O-O"
        } else {
            "O-O-O"
        });
    } else {
        let capture = board.piece_on(dest).is_some() || (piece == Piece::Pawn && changes_file);
        let name = source.to_string();
        match piece_letter(piece) {
            None if capture => san.push_str(&name[..1]),
            None => {}
            Some(letter) => {
                san.push(letter);
                // Other pieces of the same kind that can move to the same square
                let rivals: Vec<Square> = MoveGen::new_legal(board)
                    .map(|m| m.get_source())
                    .filter(|&s| s != source && board.piece_on(s) == Some(piece))
                    .filter(|&s| {
                        MoveGen::new_legal(board)
                            .any(|m| m.get_source() == s && m.get_dest() == dest)
                    })
                    .collect();
                if !rivals.is_empty() {
                    if rivals.iter().all(|r| r.get_file() != source.get_file()) {
                        san.push_str(&name[..1]);
                    } else if rivals.iter().all(|r| r.get_rank() != source.get_rank()) {
                        san.push_str(&name[1..]);
                    } else {
                        san.push_str(&name);
                    }
                }
            }
        }
        if capture {
            san.push('x');
        }
        san.push_str(&dest.to_string());
        if let Some(letter) = mv.get_promotion().and_then(piece_letter) {
            san.push('=');
            san.push(letter);
        }
    }
    let after = board.make_move_new(mv);
    if after.status() == BoardStatus::Checkmate {
        san.push('#');
    } else if after.checkers().popcnt() > 0 {
        san.push('+');
    }
    san
}

/// Escape a PGN tag value.
fn escape_tag(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Today's (UTC) date in PGN format.
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Civil date from days since the epoch (Howard Hinnant's algorithm)
    let z = (secs / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{:04}.{:02}.{:02}", year, month, day)
}

const PGN_LINE_WIDTH: usize = 79;
const UNKNOWN_DATE: &str = "????.??.??";
/// The Seven Tag Roster, with the values used when they're unknown.
const SEVEN_TAG_ROSTER: [(&str, &str); 7] = [
    ("Event", "?"),
    ("Site", "?"),
    ("Date", UNKNOWN_DATE),
    ("Round", "?"),
    ("White", "?"),
    ("Black", "?"),
    ("Result", "*"),
];

/// Number of legal half-moves that must follow an unexplained board state before the
/// current game is abandoned and a new partition is started from that state.
const MAX_FORWARD_SEARCH: usize = 8;
/// How many half-moves can be taken back and replaced with a different move.
const MAX_TAKEBACK: usize = 4;
/// Candidate partitions that haven't matched a legal move for this many updates are dropped.
const MAX_IDLE_UPDATES: usize = 4 * MAX_FORWARD_SEARCH;

/// Reconstructs a legal game from the sequence of board states recorded by the DGT board.
///
/// Every board update is matched greedily against the legal moves of the current
/// partition. Board states that can't be explained (an illegal move was made, pieces were
/// knocked over, ...) spawn candidate partitions starting from that state. If the current
/// partition doesn't recover, the first candidate to be followed by `MAX_FORWARD_SEARCH`
/// legal half-moves becomes a new partition, i.e. a variation of the game.
pub struct GameTracker {
    history: Vec<FingerPrint>,
    partitions: Vec<Partition>,
    candidates: Vec<Partition>,
    stuck: bool,
    /// PGN tags, in output order. Kept across games, except for the result.
    tags: Vec<(String, String)>,
    /// File the game is appended to when it's flushed.
    pgn_file: Option<PathBuf>,
}

impl GameTracker {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            partitions: Vec::new(),
            candidates: Vec::new(),
            stuck: false,
            tags: SEVEN_TAG_ROSTER
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            pgn_file: None,
        }
    }

    /// Record a new board state. Returns true if the recorded game changed.
    pub fn push(&mut self, game: &GameBoard) -> bool {
        self.push_fingerprint(game.fingerprint())
    }

    fn push_fingerprint(&mut self, fp: FingerPrint) -> bool {
        self.history.push(fp);
        if let Some(current) = self.partitions.last_mut() {
            let step = current.advance(&fp);
            if step != Step::Unexplained {
                self.candidates.clear();
                self.stuck = false;
                return step == Step::Moved;
            }
        } else if fp == board_fingerprint(&Board::default()) {
            self.partitions.push(Partition::new(Board::default()));
            self.candidates.clear();
            return true;
        }
        self.search_new_partition(&fp)
    }

    fn search_new_partition(&mut self, fp: &FingerPrint) -> bool {
        let castling = match self.partitions.last() {
            Some(current) => {
                let tip = current.tip();
                if !self.stuck {
                    // A player may have "passed", e.g. a move that was put back while the clock was pressed.
                    if let Some(passed) = tip.null_move() {
                        self.candidates.push(Partition::new(passed));
                    }
                }
                [
                    tip.castle_rights(Color::White),
                    tip.castle_rights(Color::Black),
                ]
            }
            None => [CastleRights::Both, CastleRights::Both],
        };
        self.stuck = true;

        for candidate in self.candidates.iter_mut() {
            candidate.advance(fp);
        }
        self.candidates
            .retain(|candidate| candidate.idle <= MAX_IDLE_UPDATES);

        for color in ALL_COLORS {
            if let Some(board) = board_from_fingerprint(fp, color, castling) {
                if !self.candidates.iter().any(|c| c.start == board) {
                    self.candidates.push(Partition::new(board));
                }
            }
        }

        // Prefer the candidate that explains the board states most directly
        if let Some((i, _)) = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.line.len() >= MAX_FORWARD_SEARCH)
            .min_by_key(|(_, c)| c.skipped)
        {
            let mut partition = self.candidates.remove(i);
            partition.continue_from(self.partitions.last().map_or(0, Partition::last_ply));
            self.partitions.push(partition);
            self.candidates.clear();
            self.stuck = false;
            return true;
        }
        false
    }

    /// Set a PGN tag, e.g. `set_tag("White", "Carlsen, Magnus")`. Tags other than the
    /// Seven Tag Roster are written after it, in the order they were first set.
    pub fn set_tag(&mut self, name: &str, value: &str) {
        match self.tags.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value.to_string(),
            None => self.tags.push((name.to_string(), value.to_string())),
        }
    }

    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Append the game to this file whenever it is flushed.
    pub fn set_pgn_file(&mut self, name: impl Into<PathBuf>) {
        self.pgn_file = Some(name.into());
    }

    /// The Result tag, or the result on the board if it's unset and the game ended in mate.
    fn result(&self) -> String {
        let result = self.tag("Result").unwrap_or("*");
        if result != "*" {
            return result.to_string();
        }
        let Some(tip) = self.partitions.last().map(Partition::tip) else {
            return result.to_string();
        };
        match (tip.status(), tip.side_to_move()) {
            (BoardStatus::Checkmate, Color::White) => "0-1",
            (BoardStatus::Checkmate, Color::Black) => "1-0",
            (BoardStatus::Stalemate, _) => "1/2-1/2",
            _ => "*",
        }
        .to_string()
    }

    /// The game as PGN. Each partition is written as a separate game with the same tags;
    /// partitions after the first start from a FEN. Only the last one carries the result.
    pub fn to_pgn(&self) -> String {
        let count = self.partitions.len();
        let result = self.result();
        let mut pgn = String::new();
        for (i, partition) in self.partitions.iter().enumerate() {
            let result = if i + 1 == count { result.as_str() } else { "*" };
            for (name, value) in &self.tags {
                let value = match (name.as_str(), value.as_str()) {
                    ("Result", _) => result.to_string(),
                    ("Date", UNKNOWN_DATE) => today(),
                    _ => value.clone(),
                };
                let _ = writeln!(pgn, "[{} \"{}\"]", name, escape_tag(&value));
            }
            if !partition.is_standard_start() {
                let _ = writeln!(pgn, "[SetUp \"1\"]");
                let _ = writeln!(pgn, "[FEN \"{}\"]", partition.fen());
            }
            let comment = match i {
                _ if count == 1 => None,
                0 => Some(format!("Part 1 of {}.", count)),
                _ => Some(format!(
                    "Part {} of {}, continued after an illegal move.",
                    i + 1,
                    count
                )),
            };
            let _ = writeln!(
                pgn,
                "\n{}\n",
                partition.movetext(comment.as_deref(), result)
            );
        }
        pgn
    }

    /// Append the game as PGN to a file, creating it if needed.
    pub fn append_pgn(&self, path: &Path) -> io::Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(self.to_pgn().as_bytes())
    }

    /// Start over, returning the partitions recorded for the finished game. If a PGN file
    /// is set and any moves were recorded, the game is appended to it first.
    pub fn flush(&mut self) -> Vec<Partition> {
        if let Some(path) = &self.pgn_file {
            if self.partitions.iter().any(|p| !p.line.is_empty()) {
                if let Err(e) = self.append_pgn(path) {
                    println!("Failed to write PGN to {}: {}", path.display(), e);
                }
            }
        }
        self.set_tag("Result", "*");
        self.history.clear();
        self.candidates.clear();
        self.stuck = false;
        std::mem::take(&mut self.partitions)
    }
}

impl fmt::Display for GameTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for partition in &self.partitions {
            writeln!(f, "{}", partition)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn game_board_from(board: &Board, start: StartPosition) -> GameBoard {
        let mut game = GameBoard {
            board: ChessBoard {
                board: [RawPiece::Empty; 64],
            },
            start,
        };
        let fp = board_fingerprint(board);
        for grid in 0..64 {
            game.board.board[grid] = piece_at(&fp, game.square_of(grid));
        }
        game
    }

    #[test]
    fn test_fingerprint_orientation() {
        let default = board_fingerprint(&Board::default());
        for start in [StartPosition::Normal, StartPosition::Mirror] {
            let mut game = game_board_from(&Board::default(), start);
            assert_eq!(game.is_starting_position(), start);
            assert_eq!(game.fingerprint(), default);
        }
    }

    /// Simulates the field updates a DGT board sends while moves are played.
    struct Sim {
        fp: FingerPrint,
        tracker: GameTracker,
    }

    impl Sim {
        fn new() -> Self {
            let fp = board_fingerprint(&Board::default());
            let mut tracker = GameTracker::new();
            tracker.push_fingerprint(fp);
            Self { fp, tracker }
        }

        fn set(&mut self, square: &str, piece: RawPiece) {
            let square = Square::from_str(square).unwrap().to_index();
            set_piece_at(&mut self.fp, square, piece);
            self.tracker.push_fingerprint(self.fp);
        }

        /// Lift the piece on `from` (and a captured piece on `to`), and put it on `to`.
        fn play(&mut self, mv: &str) {
            let (from, to) = mv.split_at(2);
            let from_index = Square::from_str(from).unwrap().to_index();
            let to_index = Square::from_str(to).unwrap().to_index();
            let piece = piece_at(&self.fp, from_index);
            if piece_at(&self.fp, to_index) != RawPiece::Empty {
                self.set(to, RawPiece::Empty);
            }
            self.set(from, RawPiece::Empty);
            self.set(to, piece);
        }

        fn moves(&self, partition: usize) -> Vec<String> {
            self.tracker.partitions[partition]
                .moves()
                .map(|mv| mv.to_string())
                .collect()
        }
    }

    #[test]
    fn test_simple_game_with_capture() {
        let mut sim = Sim::new();
        for mv in ["e2e4", "d7d5", "e4d5", "d8d5"] {
            sim.play(mv);
        }
        assert_eq!(sim.moves(0), ["e2e4", "d7d5", "e4d5", "d8d5"]);
    }

    #[test]
    fn test_move_taken_back() {
        let mut sim = Sim::new();
        sim.play("e2e4");
        sim.play("g8f6");
        sim.play("f6g8");
        sim.play("e7e5");
        sim.play("g1f3");
        sim.play("f3g5");
        assert_eq!(sim.moves(0), ["e2e4", "e7e5", "g1f3"]);
        sim.play("g5h3");
        assert_eq!(sim.moves(0), ["e2e4", "e7e5", "g1h3"]);
    }

    #[test]
    fn test_castle_rook_first() {
        let mut sim = Sim::new();
        for mv in ["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6"] {
            sim.play(mv);
        }
        sim.play("h1f1");
        sim.play("e1g1");
        assert_eq!(sim.moves(0)[6..], ["e1g1"]);
        sim.play("f8c5");
        assert_eq!(sim.moves(0).len(), 8);
        assert_eq!(sim.tracker.partitions.len(), 1);
    }

    #[test]
    fn test_illegal_move_starts_new_partition() {
        let mut sim = Sim::new();
        sim.play("e2e4");
        sim.play("e7e5");
        // Illegal knight move
        sim.play("g1g4");
        let rest = [
            "b8c6", "b1c3", "g8f6", "a2a3", "a7a6", "h2h3", "h7h6", "b2b3",
        ];
        for mv in &rest[..7] {
            sim.play(mv);
        }
        assert_eq!(sim.tracker.partitions.len(), 1);
        sim.play(rest[7]);
        assert_eq!(sim.tracker.partitions.len(), 2);
        assert_eq!(sim.moves(0), ["e2e4", "e7e5"]);
        assert_eq!(sim.moves(1), rest);
        let start = &sim.tracker.partitions[1].start;
        assert_eq!(start.side_to_move(), Color::Black);
        assert_eq!(start.castle_rights(Color::White), CastleRights::Both);
        // Play continues in the new partition
        sim.play("h6h5");
        assert_eq!(sim.moves(1).len(), 9);
    }

    #[test]
    fn test_san() {
        let san_of = |fen: &str, mv: &str| {
            let board = Board::from_str(fen).unwrap();
            let mv = chess::ChessMove::from_str(mv).unwrap();
            san(&board, mv)
        };
        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert_eq!(san_of(start, "e2e4"), "e4");
        assert_eq!(san_of(start, "g1f3"), "Nf3");
        // Knights on b1 and f3 can both go to d2
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPP2PPP/RNBQKB1R w KQkq - 0 4";
        assert_eq!(san_of(fen, "b1d2"), "Nbd2");
        // Rooks on a1 and a5 can both go to a3
        let fen = "4k3/8/8/R7/8/8/8/R3K3 w - - 0 1";
        assert_eq!(san_of(fen, "a1a3"), "R1a3");
        assert_eq!(san_of(fen, "e1c1"), "O-O-O");
        let fen = "4k3/1P6/8/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(san_of(fen, "b7b8q"), "b8=Q+");
        let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        assert_eq!(san_of(fen, "e4d5"), "exd5");
        let fen = "rnbqkbnr/ppppp2p/5p2/6p1/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3";
        assert_eq!(san_of(fen, "d1h5"), "Qh5#");
    }

    #[test]
    fn test_pgn_output() {
        let mut sim = Sim::new();
        sim.tracker.set_tag("White", "Alice");
        sim.tracker.set_tag("Date", "2026.10.20");
        for mv in ["f2f3", "e7e5", "g2g4", "d8h4"] {
            sim.play(mv);
        }
        let pgn = sim.tracker.to_pgn();
        assert!(pgn.contains("[White \"Alice\"]\n[Black \"?\"]\n[Result \"0-1\"]\n"));
        assert!(pgn.ends_with("\n1. f3 e5 2. g4 Qh4# 0-1\n\n"), "{}", pgn);
        assert!(!pgn.contains("FEN"));
    }

    #[test]
    fn test_pgn_partitions_appended_on_flush() {
        let path = std::env::temp_dir().join("jackolope_test_partitions.pgn");
        let _ = std::fs::remove_file(&path);
        let mut sim = Sim::new();
        sim.tracker.set_pgn_file(&path);
        sim.play("e2e4");
        sim.play("e7e5");
        sim.play("g1g4");
        for mv in [
            "b8c6", "b1c3", "g8f6", "a2a3", "a7a6", "h2h3", "h7h6", "b2b3",
        ] {
            sim.play(mv);
        }
        sim.tracker.flush();
        // Nothing recorded, nothing written
        sim.tracker.flush();
        let pgn = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let movetext = pgn.replace('\n', " ");
        assert_eq!(pgn.matches("[Event ").count(), 2);
        assert!(pgn.contains("{Part 1 of 2.} 1. e4 e5 *\n"));
        assert!(pgn
            .contains("[FEN \"rnbqkbnr/pppp1ppp/8/4p3/4P1N1/8/PPPP1PPP/RNBQKB1R b KQkq - 0 2\"]"));
        assert!(movetext.contains("illegal move.} 2... Nc6 3. Nc3 Nf6 4. a3 a6 5. h3 h6 6. b3 *"));
    }
}
