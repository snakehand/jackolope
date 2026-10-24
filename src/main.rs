use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use eframe::egui;
use serialport::SerialPort;

mod chessboard;
mod game;
mod protocol;

use chessboard::{initial_position, model_from_raw, Model};
use game::*;
use protocol::*;

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ResponseResult = Result<Response, BoxError>;

/// Messages from the board reader to the UI.
enum UiUpdate {
    /// The pieces currently on the board.
    Position(Model),
    /// The game recorded so far, in standard algebraic notation.
    Notation(String),
}

/// How long to wait for the board to answer a request.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

fn get_response(port: &mut Box<dyn SerialPort>) -> ResponseResult {
    let mut buffer = [0; 1];
    loop {
        port.read_exact(&mut buffer)?;
        if buffer[0] & 0x80 == 0 {
            continue;
        }
        let resp_type = buffer[0] & 0x7F;
        port.read_exact(&mut buffer)?;
        if buffer[0] & 0x80 != 0 {
            continue;
        }
        let mut length = (buffer[0] as usize) << 7;
        port.read_exact(&mut buffer)?;
        if buffer[0] & 0x80 != 0 {
            continue;
        }
        length |= buffer[0] as usize;
        if length < 3 {
            return Err("Invalid response length".into());
        }
        length -= 3;
        println!("Response type: {}, length: {}", resp_type, length);
        let mut data = Vec::with_capacity(length);
        for _ in 0..length {
            port.read_exact(&mut buffer)?;
            data.push(buffer[0]);
        }
        if let Some(rtype) = MessageType::try_from_byte(resp_type) {
            let response = match Response::try_from_raw(rtype, &data) {
                Ok(r) => r,
                Err(e) => {
                    println!(
                        "Received response: {:?}({}) but failed to parse: {:?}",
                        rtype, resp_type, e
                    );
                    return Err("Parse error".into());
                }
            };
            return Ok(response);
        } else {
            println!("Received response: Unknown({})", resp_type);
            return Err("Invalid response type".into());
        }
    }
}

/// Reads responses from the port without a timeout and forwards them, until the port
/// fails or the receiver hangs up. Parse errors are forwarded and reading continues.
fn response_reader(mut port: Box<dyn SerialPort>, responses: Sender<ResponseResult>) {
    loop {
        let response = get_response(&mut port);
        let io_error = response
            .as_ref()
            .err()
            .and_then(|e| e.downcast_ref::<io::Error>());
        // The port timeout is effectively infinite, but poll() still caps it at ~24 days
        if io_error.is_some_and(|e| e.kind() == io::ErrorKind::TimedOut) {
            continue;
        }
        let fatal = io_error.is_some();
        if responses.send(response).is_err() || fatal {
            return;
        }
    }
}

/// Wait for the next response, giving up after `timeout`. A response that arrives
/// after the timeout is delivered by the next call.
fn get_response_with_timeout(
    responses: &Receiver<ResponseResult>,
    timeout: Duration,
) -> ResponseResult {
    match responses.recv_timeout(timeout) {
        Ok(response) => response,
        Err(RecvTimeoutError::Timeout) => Err("Timed out waiting for the board".into()),
        Err(RecvTimeoutError::Disconnected) => Err("Serial port reader stopped".into()),
    }
}

/// Reads the DGT board over the serial port and sends every new position and the
/// recorded notation to the UI. Runs until the serial port fails or the UI hangs up the channel.
fn board_reader(port_name: &str, tx: Sender<UiUpdate>, ctx: egui::Context) -> Result<(), BoxError> {
    let send = |update: UiUpdate| -> bool {
        let ok = tx.send(update).is_ok();
        ctx.request_repaint();
        ok
    };
    let send_position =
        |game_board: &GameBoard| send(UiUpdate::Position(model_from_raw(game_board.squares())));
    let send_notation = |game: &GameTracker| send(UiUpdate::Notation(game.notation()));

    let mut port = serialport::new(port_name, 9600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::Hardware)
        // Block until data arrives; timeouts are handled by get_response_with_timeout
        .timeout(Duration::MAX)
        .open()?;

    // All reads go through this thread, so no partially read message is ever abandoned
    let (response_tx, responses) = mpsc::channel();
    let read_port = port.try_clone()?;
    thread::spawn(move || response_reader(read_port, response_tx));

    port.write_all(&Command::Reset.as_byte())?; // Reset the device
    port.write_all(&Command::RequestBoard.as_byte())?; // Request full board dump
    let pos = get_response_with_timeout(&responses, RESPONSE_TIMEOUT)?;
    println!("{:?}", pos);
    let mut game_board = match pos {
        Response::BoardDump(board) => {
            println!("{:?}", board);
            GameBoard::new(board)
        }
        _ => return Err("Unexpected response".into()),
    };
    port.write_all(&Command::RequestSerialNumber.as_byte())?;
    println!(
        "{:?}",
        get_response_with_timeout(&responses, RESPONSE_TIMEOUT)?
    );

    port.write_all(&Command::RequestUpdate.as_byte())?; // Request field updates

    let mut game = GameTracker::new();
    game.set_pgn_file("games.pgn");
    game.push(&game_board);
    if !send_position(&game_board) || !send_notation(&game) {
        return Ok(());
    }

    // Debug builds: the last mismatch between a board dump and the board built from field updates
    let mut last_diff = Vec::new();

    loop {
        let Ok(response) = responses.recv() else {
            return Err("Serial port reader stopped".into());
        };
        match response {
            Ok(Response::BoardDump(dump)) if cfg!(debug_assertions) => {
                // The serial stream is ordered, so every update sent before this dump
                // has already been applied to game_board.
                let diff = game_board.diff(&dump);
                if diff != last_diff {
                    if diff.is_empty() {
                        println!("BOARD CHECK: field updates match the board dump again");
                    }
                    for (square, ours, dumped) in &diff {
                        println!(
                            "BOARD CHECK: {} is {:?} from field updates, {:?} in board dump",
                            square, ours, dumped
                        );
                    }
                    last_diff = diff;
                }
            }
            Ok(response) => {
                println!("Received response: {:?}", response);
                if let Response::FieldUpdate(mv) = response {
                    if cfg!(debug_assertions) {
                        // Verify the field updates against a full dump, then make sure
                        // the board stays in update mode
                        port.write_all(&Command::RequestBoard.as_byte())?;
                        port.write_all(&Command::RequestUpdate.as_byte())?;
                    }
                    game_board.apply_move(mv);
                    // Detect the orientation before fingerprinting the board
                    let mut notation_changed = false;
                    if game_board.is_starting_position() != StartPosition::None {
                        for partition in game.flush() {
                            println!("{}", partition);
                            notation_changed = true;
                        }
                        println!("New Game");
                    }
                    if game.push(&game_board) {
                        print!("{}", game);
                        notation_changed = true;
                    }
                    if !send_position(&game_board) {
                        return Ok(());
                    }
                    if notation_changed && !send_notation(&game) {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}

struct App {
    board: chessboard::ChessBoard,
    notation: String,
    updates: Receiver<UiUpdate>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Only the latest position and notation matter; drain anything that queued up.
        while let Ok(update) = self.updates.try_recv() {
            match update {
                UiUpdate::Position(model) => self.board.model = model,
                UiUpdate::Notation(notation) => self.notation = notation,
            }
        }
        egui::Panel::right("notation")
            .resizable(true)
            .default_size(220.0)
            .min_size(120.0)
            .show(ui, |ui| {
                ui.heading("Moves");
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&self.notation).monospace())
                                .wrap(),
                        );
                    });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.board.ui(ui);
        });
    }
}

fn main() -> eframe::Result {
    let port_name = "/dev/cu.usbserial-11140";
    let light_blue = [173, 216, 230];
    let dark_blue = [30, 60, 140];

    eframe::run_native(
        "Jackolope",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([860.0, 640.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let (tx, rx) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            thread::spawn(move || {
                if let Err(e) = board_reader(port_name, tx, ctx) {
                    eprintln!("Board reader stopped: {}", e);
                }
            });
            Ok(Box::new(App {
                board: chessboard::ChessBoard::new(initial_position(), light_blue, dark_blue),
                notation: String::new(),
                updates: rx,
            }))
        }),
    )
}
