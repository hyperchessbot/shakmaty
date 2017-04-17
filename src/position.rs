use std::cmp::max;
use std::char;
use std::ascii::AsciiExt;
use std::str::FromStr;
use std::fmt;

use square;
use square::Square;
use types::{Color, White, Black, Role, Piece, Move, Uci, ROLES};
use bitboard::Bitboard;
use board::Board;
use attacks::Precomp;

pub struct RemainingChecks {
    pub white: u8,
    pub black: u8,
}

impl RemainingChecks {
    pub fn by_color(&self, color: Color) -> u8 {
        color.fold(self.white, self.black)
    }
}

impl fmt::Display for RemainingChecks {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}+{}", self.white, self.black)
    }
}

pub struct Pocket {
    pub pawns: u8,
    pub knights: u8,
    pub bishops: u8,
    pub rooks: u8,
    pub queens: u8,
    pub kings: u8,
}

impl Pocket {
    pub fn by_role(&self, role: Role) -> u8 {
        match role {
            Role::Pawn   => self.pawns,
            Role::Knight => self.knights,
            Role::Bishop => self.bishops,
            Role::Rook   => self.rooks,
            Role::Queen  => self.queens,
            Role::King   => self.kings,
        }
    }
}

pub struct Pockets {
    pub white: Pocket,
    pub black: Pocket,
}

impl Pockets {
    pub fn by_color(&self, color: Color) -> &Pocket {
        color.fold(&self.white, &self.black)
    }

    pub fn by_piece(&self, piece: Piece) -> u8 {
        self.by_color(piece.color).by_role(piece.role)
    }
}

impl fmt::Display for Pockets {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for color in &[White, Black] {
            for role in &ROLES {
                let piece = Piece { color: *color, role: *role };
                try!(write!(f, "{}", piece.char().to_string().repeat(self.by_piece(piece) as usize)));
            }
        }
        Ok(())
    }
}

pub trait Position : Clone + Default {
    const MAX_LEGAL_MOVES: usize;

    fn board(&self) -> &Board;
    fn pockets(&self) -> Option<&Pockets> { None }
    fn turn(&self) -> Color;
    fn castling_rights(&self) -> Bitboard;
    fn ep_square(&self) -> Option<Square>;
    fn remaining_checks(&self) -> Option<&RemainingChecks> { None }
    fn halfmove_clock(&self) -> u32;
    fn fullmoves(&self) -> u32;

    fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.board().piece_at(sq)
    }

    fn fen(&self) -> String {
        let pockets = self.pockets()
                          .map_or("".to_owned(), |p| format!("[{}]", p));

        let checks = self.remaining_checks()
                         .map_or("".to_owned(), |r| format!(" {}", r));

        format!("{}{} {} {} {}{} {} {}",
                self.board().board_fen(self.pockets().is_some()),
                pockets,
                self.turn().char(),
                self.castling_xfen(),
                self.ep_square().map_or("-".to_owned(), |sq| sq.to_string()),
                checks,
                self.halfmove_clock(),
                self.fullmoves())
    }

    fn castling_xfen(&self) -> String {
        let mut fen = String::with_capacity(4);

        for color in &[White, Black] {
            let king = self.board().king_of(*color);

            let candidates = self.board().by_piece(color.rook()) &
                             Bitboard::relative_rank(*color, 0);

            for rook in (candidates & self.castling_rights()).rev() {
                if Some(rook) == candidates.first() && king.map_or(false, |k| rook < k) {
                    fen.push(color.fold('Q', 'q'));
                } else if Some(rook) == candidates.last() && king.map_or(false, |k| k < rook) {
                    fen.push(color.fold('K', 'k'));
                } else {
                    fen.push((rook.file() as u8 + color.fold('A', 'a') as u8) as char);
                }
            }
        }

        if fen.is_empty() {
            fen.push('-');
        }

        fen
    }

    fn san_candidates(&self, moves: &mut Vec<Move>, role: Role, target: Square, precomp: &Precomp) {
        self.legal_moves(moves, precomp);
        moves.retain(|m| match *m {
            Move::Normal { from, to, .. } =>
                to == target && self.board().by_piece(role.of(self.turn())).contains(from),
            _ => false
        })
    }

    fn san(self, m: &Move, precomp: &Precomp) -> String {
        fn suffix<P: Position>(pos: P, m: &Move, precomp: &Precomp) -> &'static str {
            let after = pos.do_move(m);

            if after.checkers(precomp).is_empty() {
                ""
            } else {
                let mut legals = Vec::new();
                after.legal_moves(&mut legals, precomp);
                if legals.is_empty() { "#" } else { "+" }
            }
        }

        match *m {
            Move::Normal { role, from, capture, to, promotion } => {
                let mut san = String::new();

                if role != Role::Pawn {
                    san.push(role.char().to_ascii_uppercase());

                    // Disambiguate.
                    let mut legals = Vec::new();
                    self.san_candidates(&mut legals, role, to, precomp);

                    let (rank, file) = legals.iter().fold((false, false), |(rank, file), c| match *c {
                        Move::Normal { from: candidate, .. } =>
                            if from == candidate {
                                (rank, file)
                            } else if from.rank() == candidate.rank() {
                                (rank, true)
                            } else if from.file() == candidate.file() {
                                (true, file)
                            } else {
                                (rank, true)
                            },
                        _ => (rank, file)
                    });

                    if file {
                        san.push(from.file_char());
                    }
                    if rank {
                        san.push(from.rank_char());
                    }
                }

                if capture.is_some() {
                    if role == Role::Pawn {
                        san.push(from.file_char())
                    }
                    san.push('x');
                }

                san.push_str(&to.to_string());

                promotion.map(|r| {
                    san.push('=');
                    san.push(r.char().to_ascii_uppercase());
                });

                san.push_str(suffix(self, m, precomp));

                san
            },
            Move::EnPassant { from, to, .. } => format!("{}x{}{}", from.file_char(), to, suffix(self, m, precomp)),
            Move::Castle { .. } => format!("{}{}", m, suffix(self, m, precomp)),
            Move::Put { to, role } => format!("{}@{}{}", role.char().to_ascii_uppercase(), to, suffix(self, m, precomp)),
            Move::Null => "--".to_owned()
        }
    }

    fn checkers(&self, precomp: &Precomp) -> Bitboard {
        self.board().king_of(self.turn())
            .map_or(Bitboard(0), |king| self.board().by_color(!self.turn()) & self.board().attacks_to(king, precomp))
    }

    fn legal_moves(&self, moves: &mut Vec<Move>, precomp: &Precomp);

    fn do_move(self, m: &Move) -> Self;

    fn validate(&self, uci: &Uci) -> Option<Move> {
        match *uci {
            Uci::Normal { from, to, promotion } => {
                self.board().role_at(from).map(|role| {
                    Move::Normal { role, from, capture: self.board().role_at(to), to, promotion }
                })
            },
            Uci::Put { role, to } => Some(Move::Put { role, to }),
            Uci::Null => Some(Move::Null)
        }
    }
}

#[derive(Clone)]
pub struct Standard {
    board: Board,

    turn: Color,
    castling_rights: Bitboard,
    ep_square: Option<Square>,

    halfmove_clock: u32,
    fullmoves: u32,
}

impl Position for Standard {
    const MAX_LEGAL_MOVES: usize = 255;

    fn board(&self) -> &Board { &self.board }
    fn turn(&self) -> Color { self.turn }
    fn castling_rights(&self) -> Bitboard { self.castling_rights }
    fn ep_square(&self) -> Option<Square> { self.ep_square }
    fn halfmove_clock(&self) -> u32 { self.halfmove_clock }
    fn fullmoves(&self) -> u32 { self.fullmoves }

    fn legal_moves(&self, moves: &mut Vec<Move>, precomp: &Precomp) {
        if self.checkers(precomp).is_empty() {
            self.gen_pseudo_legal(Bitboard::all(), Bitboard::all(), moves, precomp);
            self.gen_en_passant(moves, precomp);
            self.gen_castling_moves(moves, precomp);
        } else {
            self.evasions(moves, precomp);
        }

        let blockers = self.slider_blockers(self.them(),
                                            self.board.king_of(self.turn()).unwrap(),
                                            precomp);

        moves.retain(|m| self.is_safe(m, blockers, precomp));
    }

    fn do_move(mut self, m: &Move) -> Standard {
        let color = self.turn();
        self.ep_square().take();
        self.halfmove_clock += 1;

        match *m {
            Move::Normal { role, from, capture, to, promotion } => {
                if role == Role::Pawn || capture.is_some() {
                    self.halfmove_clock = 0;
                }

                if role == Role::Pawn && square::distance(from, to) == 2 {
                    self.ep_square = from.offset(color.fold(8, -8));
                }

                if role == Role::King {
                    self.castling_rights.discard_all(Bitboard::relative_rank(color, 0));
                } else {
                    self.castling_rights.discard(from);
                    self.castling_rights.discard(to);
                }

                let promoted = self.board.promoted().remove(from) || promotion.is_some();

                self.board.set_piece_at(to, promotion.map(|p| p.of(color))
                                                     .unwrap_or(role.of(color)));

                if promoted {
                    self.board.promoted().flip(to);
                }
            },
            Move::Castle { king, rook } => {
                let rook_to = Square::from_coords(
                    if square::delta(rook, king) < 0 { 3 } else { 5 },
                    color.fold(0, 7)).unwrap();

                let king_to = Square::from_coords(
                    if square::delta(rook, king) < 0 { 2 } else { 6 },
                    color.fold(0, 7)).unwrap();

                self.board.remove_piece_at(king);
                self.board.remove_piece_at(rook);
                self.board.set_piece_at(rook_to, color.rook());
                self.board.set_piece_at(king_to, color.king());

                self.castling_rights.discard_all(Bitboard::relative_rank(color, 0));
            },
            Move::EnPassant { from, to, pawn } => {
                self.board.remove_piece_at(pawn);
                self.board.remove_piece_at(from).map(|piece| self.board.set_piece_at(to, piece));
                self.halfmove_clock = 0;
            },
            Move::Put { to, role } => {
                self.board.set_piece_at(to, Piece { color, role });
            },
            Move::Null => ()
        }

        self.turn = !self.turn;

        if self.turn() == White {
            self.fullmoves += 1;
        }

        self
    }
}

impl Default for Standard {
    fn default() -> Self {
        Standard {
            board: Board::default(),

            turn: White,
            castling_rights: Bitboard(0x8100000000000081),
            ep_square: None,

            halfmove_clock: 0,
            fullmoves: 1,
        }
    }
}

impl Standard {
    pub fn empty() -> Standard {
        Standard {
            board: Board::empty(),

            turn: White,
            castling_rights: Bitboard(0),
            ep_square: None,

            halfmove_clock: 0,
            fullmoves: 1,
        }
    }

    pub fn from_fen(fen: &str) -> Option<Standard> {
        let mut pos = Standard::empty();
        let mut parts = fen.split(' ');

        if let Some(board) = parts.next().and_then(|board_fen| Board::from_board_fen(board_fen)) {
            pos.board = board
        } else {
            return None
        }

        match parts.next() {
            Some("w") => pos.turn = White,
            Some("b") => pos.turn = Black,
            Some(_)   => return None,
            None      => ()
        }

        if let Some(castling_part) = parts.next() {
            for ch in castling_part.chars() {
                if ch == '-' {
                    continue;
                }

                let color = Color::from_bool(ch.to_ascii_uppercase() == ch);

                let candidates = Bitboard::relative_rank(color, 0) &
                                 pos.board.by_piece(Role::Rook.of(color));

                let flag = match ch.to_ascii_lowercase() {
                    'k'  => candidates.last(),
                    'q'  => candidates.first(),
                    file => (candidates & Bitboard::file(file as i8 - 'a' as i8)).first(),
                };

                match flag {
                    Some(cr) => pos.castling_rights.add(cr),
                    None     => return None
                }
            }
        }

        if let Some(ep_part) = parts.next() {
            if ep_part != "-" {
                match Square::from_str(ep_part) {
                    Ok(sq) => pos.ep_square = Some(sq),
                    _      => return None
                }
            }
        }

        if let Some(halfmoves_part) = parts.next() {
            match halfmoves_part.parse::<u32>() {
                Ok(halfmoves) => pos.halfmove_clock = halfmoves,
                _             => return None
            }
        }

        if let Some(fullmoves_part) = parts.next() {
            match fullmoves_part.parse::<u32>() {
                Ok(fullmoves) => pos.fullmoves = max(1, fullmoves),
                _             => return None
            }
        }

        Some(pos)
    }

    pub fn us(&self) -> Bitboard {
        self.board.by_color(self.turn)
    }

    pub fn our(&self, role: Role) -> Bitboard {
        self.us() & self.board.by_role(role)
    }

    pub fn them(&self) -> Bitboard {
        self.board.by_color(!self.turn)
    }

    fn push_pawn_moves(&self, moves: &mut Vec<Move>, from: Square, to: Square) {
        let capture = self.board.role_at(to); // XXX

        if to.rank() == self.turn.fold(7, 0) {
            moves.push(Move::Normal { role: Role::Pawn, from, capture, to, promotion: Some(Role::Queen) } );
            moves.push(Move::Normal { role: Role::Pawn, from, capture, to, promotion: Some(Role::Rook) } );
            moves.push(Move::Normal { role: Role::Pawn, from, capture, to, promotion: Some(Role::Bishop) } );
            moves.push(Move::Normal { role: Role::Pawn, from, capture, to, promotion: Some(Role::Knight) } );
        } else {
            moves.push(Move::Normal { role: Role::Pawn, from, capture, to, promotion: None } );
        }
    }

    fn push_moves(&self, moves: &mut Vec<Move>, role: Role, from: Square, to: Bitboard) {
        for square in to {
            moves.push(Move::Normal { role, from, capture: self.board.role_at(square), to: square, promotion: None });
        }
    }

    fn gen_pseudo_legal(&self, selection: Bitboard, target: Bitboard, moves: &mut Vec<Move>, precomp: &Precomp) {
        for from in self.our(Role::King) & selection {
            self.push_moves(moves, Role::King, from,
                            precomp.king_attacks(from) & !self.us() & target);
        }

        for from in self.our(Role::Knight) & selection {
            self.push_moves(moves, Role::Knight, from,
                            precomp.knight_attacks(from) & !self.us() & target);
        }

        for from in self.our(Role::Rook) & selection {
            self.push_moves(moves, Role::Rook, from,
                            precomp.rook_attacks(from, self.board.occupied()) & !self.us() & target);
        }

        for from in self.our(Role::Queen) & selection {
            self.push_moves(moves, Role::Queen, from,
                            precomp.rook_attacks(from, self.board.occupied()) & !self.us() & target);
        }

        for from in self.our(Role::Bishop) & selection {
            self.push_moves(moves, Role::Bishop, from,
                            precomp.bishop_attacks(from, self.board.occupied()) & !self.us() & target);
        }

        for from in self.our(Role::Queen) & selection {
            self.push_moves(moves, Role::Queen, from,
                            precomp.bishop_attacks(from, self.board.occupied()) & !self.us() & target);
        }

        for from in self.our(Role::Pawn) {
            for to in precomp.pawn_attacks(self.turn, from) & self.them() & target {
                self.push_pawn_moves(moves, from, to);
            }
        }

        let single_moves = (self.our(Role::Pawn) & selection).relative_shift(self.turn, 8) &
                           !self.board.occupied();

        let double_moves = single_moves.relative_shift(self.turn, 8) &
                           Bitboard::relative_rank(self.turn, 3) &
                           !self.board.occupied();

        for to in single_moves & target {
            if let Some(from) = to.offset(self.turn.fold(-8, 8)) {
                self.push_pawn_moves(moves, from, to);
            }
        }

        for to in double_moves & target {
            if let Some(from) = to.offset(self.turn.fold(-16, 16)) {
                self.push_pawn_moves(moves, from, to);
            }
        }
    }

    fn gen_en_passant(&self, moves: &mut Vec<Move>, precomp: &Precomp) {
        if let Some(to) = self.ep_square {
            for from in self.our(Role::Pawn) & precomp.pawn_attacks(!self.turn, to) {
                moves.push(Move::EnPassant { from, to, pawn: to.offset(self.turn.fold(-8, 8)).unwrap() }); // XXX
            }
        }
    }

    fn slider_blockers(&self, sliders: Bitboard, sq: Square, precomp: &Precomp) -> Bitboard {
        let snipers = (precomp.rook_attacks(sq, Bitboard(0)) & self.board.rooks_and_queens()) |
                      (precomp.bishop_attacks(sq, Bitboard(0)) & self.board.bishops_and_queens());

        let mut blockers = Bitboard(0);

        for sniper in snipers & sliders {
            let b = precomp.between(sq, sniper) & self.board.occupied();

            if !b.more_than_one() {
                blockers = blockers | b;
            }
        }

        blockers
    }

    fn is_safe(&self, m: &Move, blockers: Bitboard, precomp: &Precomp) -> bool {
        match *m {
            Move::Normal { role, from, to, .. } =>
                if role == Role::King {
                    (self.board.attacks_to(to, precomp) & self.them()).is_empty()
                } else {
                    !(self.us() & blockers).contains(from) ||
                    precomp.aligned(from, to, self.our(Role::King).first().unwrap())
                },
            Move::EnPassant { from, to, pawn } => {
                let mut occupied = self.board.occupied();
                occupied.flip(from);
                occupied.flip(pawn);
                occupied.add(to);

                self.our(Role::King).first().map(|king| {
                    (precomp.rook_attacks(king, occupied) & self.them() & self.board.rooks_and_queens()).is_empty() &&
                    (precomp.bishop_attacks(king, occupied) & self.them() & self.board.bishops_and_queens()).is_empty()
                }).unwrap_or(true)
            },
            Move::Castle { .. } => {
                true
            },
            _ => false // XXX
        }
    }

    fn evasions(&self, moves: &mut Vec<Move>, precomp: &Precomp) {
        let checkers = self.checkers(precomp);
        let king = self.our(Role::King).first().unwrap();
        let sliders = checkers & self.board.sliders();

        let mut attacked = Bitboard(0);
        for checker in sliders {
            attacked = attacked | precomp.ray(checker, king).without(checker);
        }

        for to in precomp.king_attacks(king) & !self.us() & !attacked {
            moves.push(Move::Normal { role: Role::King, from: king, capture: self.board.role_at(to), to, promotion: None });
        }

        if let Some(checker) = checkers.single_square() {
            let target = precomp.between(king, checker).with(checker);
            self.gen_pseudo_legal(!self.board.kings(), target, moves, precomp);
            self.gen_en_passant(moves, precomp);
        }
    }

    fn gen_castling_moves(&self, moves: &mut Vec<Move>, precomp: &Precomp) {
        let backrank = Bitboard::relative_rank(self.turn, 0);

        for king in self.our(Role::King) & backrank {
            'next_rook: for rook in self.castling_rights & backrank {
                let (king_to, rook_to) = if king < rook {
                    (self.turn.fold(square::G1, square::G8),
                     self.turn.fold(square::H1, square::H8))
                } else {
                    (self.turn.fold(square::C1, square::C8),
                     self.turn.fold(square::D1, square::D8))
                };

                let empty_for_king = precomp.between(king, king_to).with(king_to)
                                            .without(rook).without(king);

                let empty_for_rook = precomp.between(rook, rook_to).with(rook_to)
                                            .without(rook).without(king);

                if !(self.board.occupied() & empty_for_king).is_empty() {
                    continue;
                }

                if !(self.board.occupied() & empty_for_rook).is_empty() {
                    continue;
                }

                for sq in precomp.between(king, king_to).with(king).with(king_to) {
                    if !(self.board.attacks_to(sq, precomp) & self.them()).is_empty() {
                        continue 'next_rook;
                    }
                }

                if !(precomp.rook_attacks(king_to, self.board.occupied().without(rook)) &
                     self.them() & self.board.rooks_and_queens()).is_empty() {
                    continue;
                }

                moves.push(Move::Castle { king, rook });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use square;

    #[test]
    fn test_castling_moves() {
        let precomp = Precomp::new();

        let fen = "rnbqkbnr/pppppppp/8/8/8/5NP1/PPPPPPBP/RNBQK2R w KQkq - 0 1";
        let pos = Standard::from_fen(fen).unwrap();

        let castle = pos.validate(&Uci::from_str("e1g1").unwrap()).unwrap();
        let mut moves = Vec::new();
        pos.legal_moves(&mut moves, &precomp);
        assert!(moves.contains(&castle));

        let pos = pos.do_move(&castle);
        assert_eq!(pos.piece_at(square::G1), Some(White.king()));
        assert_eq!(pos.piece_at(square::F1), Some(White.rook()));
    }

    #[test]
    fn test_fen() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let pos = Standard::from_fen(fen).unwrap();
        assert_eq!(pos.fen(), fen);

        let fen = "4k3/8/8/8/8/8/8/4K2R w K - 0 1";
        let pos = Standard::from_fen(fen).unwrap();
        assert_eq!(pos.fen(), fen);
    }

    #[test]
    fn test_san() {
        let precomp = Precomp::new();
        let pos = Standard::default();
        let m = Move::Normal { role: Role::Knight, from: square::G1, capture: None, to: square::F3, promotion: None };
        assert_eq!(pos.san(&m, &precomp), "Nf3");
    }
}
