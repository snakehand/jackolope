use crate::protocol::RawPiece;
use eframe::egui;
use resvg::{tiny_skia, usvg};

const SPRITE_SVG: &[u8] = include_bytes!("../Chess_Pieces_Sprite.svg");
/// Size of one piece in the SVG's own coordinate system.
const SVG_CELL: f32 = 45.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
}

impl Kind {
    /// Column of this piece in the sprite sheet.
    fn sprite_col(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Piece {
    #[default]
    Empty,
    White(Kind),
    Black(Kind),
}

impl From<RawPiece> for Piece {
    fn from(raw: RawPiece) -> Self {
        use Kind::*;
        match raw {
            RawPiece::Empty => Piece::Empty,
            RawPiece::WhitePawn => Piece::White(Pawn),
            RawPiece::WhiteRook => Piece::White(Rook),
            RawPiece::WhiteKnight => Piece::White(Knight),
            RawPiece::WhiteBishop => Piece::White(Bishop),
            RawPiece::WhiteKing => Piece::White(King),
            RawPiece::WhiteQueen => Piece::White(Queen),
            RawPiece::BlackPawn => Piece::Black(Pawn),
            RawPiece::BlackRook => Piece::Black(Rook),
            RawPiece::BlackKnight => Piece::Black(Knight),
            RawPiece::BlackBishop => Piece::Black(Bishop),
            RawPiece::BlackKing => Piece::Black(King),
            RawPiece::BlackQueen => Piece::Black(Queen),
        }
    }
}

/// 64 squares, index = rank * 8 + file, where index 0 is a1 and 63 is h8.
pub type Model = [Piece; 64];

/// Convert a board in canonical orientation (see `GameBoard::squares`) to a UI model.
pub fn model_from_raw(squares: [RawPiece; 64]) -> Model {
    squares.map(Piece::from)
}

pub fn initial_position() -> Model {
    use Kind::*;
    let back = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];
    let mut m = [Piece::Empty; 64];
    for file in 0..8 {
        m[file] = Piece::White(back[file]);
        m[8 + file] = Piece::White(Pawn);
        m[48 + file] = Piece::Black(Pawn);
        m[56 + file] = Piece::Black(back[file]);
    }
    m
}

/// Sprite sheet rendered at a given square size, pre-composited on both square colours.
struct Sheets {
    square_px: usize,
    on_light: tiny_skia::Pixmap,
    on_dark: tiny_skia::Pixmap,
}

pub struct ChessBoard {
    pub model: Model,
    light: [u8; 3],
    dark: [u8; 3],
    svg: usvg::Tree,
    sheets: Option<Sheets>,
    texture: Option<egui::TextureHandle>,
    /// Model + square size the current texture was built from.
    rendered: Option<(Model, usize)>,
}

impl ChessBoard {
    pub fn new(model: Model, light: [u8; 3], dark: [u8; 3]) -> Self {
        let svg = usvg::Tree::from_data(SPRITE_SVG, &usvg::Options::default())
            .expect("valid chess sprite svg");
        Self {
            model,
            light,
            dark,
            svg,
            sheets: None,
            texture: None,
            rendered: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let avail = ui.available_size();
        let side_pts = avail.x.min(avail.y).max(8.0);
        let ppp = ui.ctx().pixels_per_point();
        let square_px = ((side_pts * ppp) / 8.0).floor().max(1.0) as usize;

        if self.rendered != Some((self.model, square_px)) {
            let image = self.render_board(square_px);
            match &mut self.texture {
                Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        "chessboard",
                        image,
                        egui::TextureOptions::NEAREST,
                    ))
                }
            }
            self.rendered = Some((self.model, square_px));
        }

        // Display at exact pixel size so the bitmap maps 1:1 onto the screen.
        let size = egui::Vec2::splat((square_px * 8) as f32 / ppp);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        if let Some(tex) = &self.texture {
            ui.painter().image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        response
    }

    fn sheets(&mut self, square_px: usize) -> &Sheets {
        if self.sheets.as_ref().map(|s| s.square_px) != Some(square_px) {
            let scale = square_px as f32 / SVG_CELL;
            let mut pieces =
                tiny_skia::Pixmap::new((square_px * 6) as u32, (square_px * 2) as u32).unwrap();
            resvg::render(
                &self.svg,
                tiny_skia::Transform::from_scale(scale, scale),
                &mut pieces.as_mut(),
            );
            let on_bg = |[r, g, b]: [u8; 3]| {
                let mut p = pieces.clone();
                p.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
                p.draw_pixmap(
                    0,
                    0,
                    pieces.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
                p
            };
            self.sheets = Some(Sheets {
                square_px,
                on_light: on_bg(self.light),
                on_dark: on_bg(self.dark),
            });
        }
        self.sheets.as_ref().unwrap()
    }

    fn render_board(&mut self, square_px: usize) -> egui::ColorImage {
        let (light, dark, model) = (self.light, self.dark, self.model);
        let sheets = self.sheets(square_px);
        let side = square_px * 8;
        let mut out = vec![0u8; side * side * 4];

        for row in 0..8 {
            let rank = 7 - row; // rank 8 at the top
            for file in 0..8 {
                let is_light = (rank + file) % 2 == 1;
                let (sheet, bg) = if is_light {
                    (&sheets.on_light, light)
                } else {
                    (&sheets.on_dark, dark)
                };
                let sprite = match model[rank * 8 + file] {
                    Piece::Empty => None,
                    Piece::White(k) => Some((k.sprite_col(), 0)),
                    Piece::Black(k) => Some((k.sprite_col(), 1)),
                };
                for y in 0..square_px {
                    let dst_start = ((row * square_px + y) * side + file * square_px) * 4;
                    let dst = &mut out[dst_start..dst_start + square_px * 4];
                    match sprite {
                        Some((col, srow)) => {
                            let stride = sheet.width() as usize * 4;
                            let src_start = (srow * square_px + y) * stride + col * square_px * 4;
                            dst.copy_from_slice(&sheet.data()[src_start..src_start + square_px * 4]);
                        }
                        None => {
                            for px in dst.chunks_exact_mut(4) {
                                px.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
                            }
                        }
                    }
                }
            }
        }
        // Sheets are fully opaque, so premultiplied == unmultiplied.
        egui::ColorImage::from_rgba_unmultiplied([side, side], &out)
    }
}
