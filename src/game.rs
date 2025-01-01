use crate::protocol::*;

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

    pub fn is_starting_position(&self) -> StartPosition {
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
            return StartPosition::Normal;
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
            return StartPosition::Mirror;
        }

        StartPosition::None
    }
}
