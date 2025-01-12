// This file is part of the shakmaty library.
// Copyright (C) 2017-2019 Niklas Fiekas <niklas.fiekas@backscattering.de>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

//! Parse and write moves in Universal Chess Interface representation.
//!
//! # Examples
//!
//! Parsing UCIs:
//!
//! ```
//! # use std::error::Error;
//! #
//! # use shakmaty::Square;
//! use shakmaty::uci::Uci;
//!
//! let uci: Uci = "g1f3".parse()?;
//!
//! assert_eq!(uci, Uci::Normal {
//!     from: Square::G1,
//!     to: Square::F3,
//!     promotion: None,
//! });
//! #
//! # Ok::<_, Box<dyn Error>>(())
//! ```
//!
//! Converting to a legal move in the context of a position:
//!
//! ```
//! # use std::error::Error;
//! #
//! # use shakmaty::Color::White;
//! # use shakmaty::uci::Uci;
//! use shakmaty::{Square, Chess, Setup, Position};
//!
//! # let uci: Uci = "g1f3".parse()?;
//! let mut pos = Chess::default();
//! let m = uci.to_move(&pos)?;
//!
//! pos.play_unchecked(&m);
//! assert_eq!(pos.board().piece_at(Square::F3), Some(White.knight()));
//! #
//! # Ok::<_, Box<dyn Error>>(())
//! ```
//!
//! Converting from [`Move`] to [`Uci`]:
//!
//! ```
//! # use shakmaty::{Square, Move, Role, Chess, Position};
//! # use shakmaty::uci::Uci;
//! #
//! let pos = Chess::default();
//!
//! let m = Move::Normal {
//!     role: Role::Knight,
//!     from: Square::B1,
//!     to: Square::C3,
//!     capture: None,
//!     promotion: None,
//! };
//!
//! let uci = m.to_uci(pos.castles().mode());
//! assert_eq!(uci.to_string(), "b1c3");
//!
//! let uci = Uci::from_standard(&m);
//! assert_eq!(uci.to_string(), "b1c3");
//!
//! let uci = Uci::from_chess960(&m);
//! assert_eq!(uci.to_string(), "b1c3");
//! ```
//!
//! [`Move`]: super::Move

use std::fmt;
use std::str::FromStr;
use std::error::Error;

use crate::square::{Rank, Square};
use crate::types::{CastlingMode, CastlingSide, Move, Role};
use crate::position::Position;

/// Error when parsing an invalid UCI.
#[derive(Clone, Debug)]
pub struct ParseUciError;

impl fmt::Display for ParseUciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "invalid uci".fmt(f)
    }
}

impl Error for ParseUciError {
    fn description(&self) -> &str {
        "invalid uci"
    }
}

/// Error when UCI is illegal.
#[derive(Clone, Debug)]
pub struct IllegalUciError;

impl fmt::Display for IllegalUciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "illegal uci".fmt(f)
    }
}

impl Error for IllegalUciError {
    fn description(&self) -> &str {
        "illegal uci"
    }
}

/// A move as represented in the UCI protocol.
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub enum Uci {
    /// A normal move, e.g. `e2e4` or `h2h1q`.
    Normal {
        from: Square,
        to: Square,
        promotion: Option<Role>,
    },
    /// A piece drop, e.g. `Q@f7`.
    Put { role: Role, to: Square },
    /// A null move (`0000`).
    Null,
}

impl FromStr for Uci {
    type Err = ParseUciError;

    fn from_str(uci: &str) -> Result<Uci, ParseUciError> {
        Uci::from_ascii(uci.as_bytes())
    }
}

impl fmt::Display for Uci {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Uci::Normal { from, to, promotion: None } =>
                write!(f, "{}{}", from, to),
            Uci::Normal { from, to, promotion: Some(promotion) } =>
                write!(f, "{}{}{}", from, to, promotion.char()),
            Uci::Put { to, role } =>
                write!(f, "{}@{}", role.upper_char(), to),
            Uci::Null =>
                write!(f, "0000")
        }
    }
}

impl Uci {
    /// Parses a move in UCI notation.
    ///
    /// # Errors
    ///
    /// Returns [`ParseUciError`] if `uci` is not syntactically valid.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// #
    /// use shakmaty::uci::Uci;
    /// use shakmaty::Square;
    ///
    /// let uci = Uci::from_ascii(b"e4e5")?;
    ///
    /// assert_eq!(uci, Uci::Normal {
    ///     from: Square::E4,
    ///     to: Square::E5,
    ///     promotion: None,
    /// });
    /// #
    /// # Ok::<_, Box<dyn Error>>(())
    /// ```
    pub fn from_ascii(uci: &[u8]) -> Result<Uci, ParseUciError> {
        if uci.len() != 4 && uci.len() != 5 {
            return Err(ParseUciError);
        }

        if uci == b"0000" {
            return Ok(Uci::Null);
        }

        let to = Square::from_ascii(&uci[2..4]).map_err(|_| ParseUciError)?;

        if uci[1] == b'@' {
            Ok(Uci::Put { role: Role::from_char(char::from(uci[0])).ok_or(ParseUciError)?, to })
        } else {
            let from = Square::from_ascii(&uci[0..2]).map_err(|_| ParseUciError)?;
            if uci.len() == 5 {
                Ok(Uci::Normal {
                    from,
                    to,
                    promotion: Some(Role::from_char(char::from(uci[4])).ok_or(ParseUciError)?)
                })
            } else {
                Ok(Uci::Normal { from, to, promotion: None })
            }
        }
    }

    /// Converts a move to UCI notation. Castling moves are represented as
    /// a move of the king to its new position.
    ///
    /// Warning: Using standard notation for castling moves in Chess960 may
    /// create moves that are illegal or moves that can be confused with
    /// king moves.
    ///
    /// # Examples
    ///
    /// ```
    /// use shakmaty::uci::Uci;
    /// use shakmaty::{Move, Square};
    ///
    /// let m = Move::Castle {
    ///     king: Square::E8,
    ///     rook: Square::H8,
    /// };
    ///
    /// let uci = Uci::from_standard(&m);
    /// assert_eq!(uci.to_string(), "e8g8");
    /// ```
    pub fn from_standard(m: &Move) -> Uci {
        match *m {
            Move::Castle { king, rook } => {
                let side = CastlingSide::from_king_side(king < rook);
                Uci::Normal {
                    from: king,
                    to: Square::from_coords(side.king_to_file(), king.rank()),
                    promotion: None,
                }
            }
            _ => Uci::from_chess960(m)
        }
    }

    /// Converts a move to UCI notation. Castling moves are represented as
    /// a move of the king to the corresponding rook square, independently of
    /// the position.
    ///
    /// # Examples
    ///
    /// ```
    /// use shakmaty::uci::Uci;
    /// use shakmaty::{Move, Square};
    ///
    /// let m = Move::Castle {
    ///     king: Square::E8,
    ///     rook: Square::H8,
    /// };
    ///
    /// let uci = Uci::from_chess960(&m);
    /// assert_eq!(uci.to_string(), "e8h8");
    /// ```
    pub fn from_chess960(m: &Move) -> Uci {
        match *m {
            Move::Normal { from, to, promotion, .. } =>
                Uci::Normal { from, to, promotion },
            Move::EnPassant { from, to, .. } =>
                Uci::Normal { from, to, promotion: None },
            Move::Castle { king, rook } =>
                Uci::Normal { from: king, to: rook, promotion: None }, // Chess960-style
            Move::Put { role, to } =>
                Uci::Put { role, to },
        }
    }

    /// See [`Uci::from_standard()`] or [`Uci::from_chess960()`].
    pub fn from_move(m: &Move, mode: CastlingMode) -> Uci {
        match mode {
            CastlingMode::Standard => Uci::from_standard(m),
            CastlingMode::Chess960 => Uci::from_chess960(m),
        }
    }

    /// Tries to convert the `Uci` to a legal [`Move`] in the context of a
    /// position.
    ///
    /// # Errors
    ///
    /// Returns [`IllegalUciError`] if the move is not legal.
    ///
    /// [`Move`]: super::Move
    pub fn to_move<P: Position>(&self, pos: &P) -> Result<Move, IllegalUciError> {
        let candidate = match *self {
            Uci::Normal { from, to, promotion } => {
                let role = pos.board().role_at(from).ok_or(IllegalUciError)?;

                if promotion.is_some() && role != Role::Pawn {
                    return Err(IllegalUciError)
                }

                if role == Role::King && pos.castling_rights().contains(to) {
                    Move::Castle { king: from, rook: to }
                } else if role == Role::King &&
                          from == pos.turn().fold(Square::E1, Square::E8) &&
                          to.rank() == pos.turn().fold(Rank::First, Rank::Eighth) &&
                          from.distance(to) == 2 {
                    if from.file() < to.file() {
                        Move::Castle { king: from, rook: pos.turn().fold(Square::H1, Square::H8) }
                    } else {
                        Move::Castle { king: from, rook: pos.turn().fold(Square::A1, Square::A8) }
                    }
                } else if role == Role::Pawn &&
                          from.file() != to.file() &&
                          !pos.board().occupied().contains(to) {
                    Move::EnPassant { from, to }
                } else {
                    Move::Normal { role, from, capture: pos.board().role_at(to), to, promotion }
                }
            },
            Uci::Put { role, to } => Move::Put { role, to },
            Uci::Null => return Err(IllegalUciError)
        };

        if pos.is_legal(&candidate) {
            Ok(candidate)
        } else {
            Err(IllegalUciError)
        }
    }
}

impl Move {
    /// See [`Uci::from_move()`].
    pub fn to_uci(&self, mode: CastlingMode) -> Uci {
        Uci::from_move(self, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Chess;
    use crate::position::Crazyhouse;

    #[test]
    pub fn test_uci_to_en_passant() {
        let mut pos = Chess::default();
        let e4 = "e2e4".parse::<Uci>().expect("e4").to_move(&pos).expect("legal");
        pos.play_unchecked(&e4);
        let nc6 = "b8c6".parse::<Uci>().expect("Nc6").to_move(&pos).expect("legal");
        pos.play_unchecked(&nc6);
        let e5 = "e4e5".parse::<Uci>().expect("e5").to_move(&pos).expect("legal");
        pos.play_unchecked(&e5);
        let d5 = "d7d5".parse::<Uci>().expect("d5").to_move(&pos).expect("legal");
        pos.play_unchecked(&d5);
        let exd5 = "e5d6".parse::<Uci>().expect("exd6").to_move(&pos).expect("legal en passant");
        assert!(exd5.is_en_passant());
    }

    #[test]
    pub fn test_uci_to_crazyhouse() {
        let mut pos = Crazyhouse::default();
        let e4 = "e2e4".parse::<Uci>().expect("e4").to_move(&pos).expect("legal");
        pos.play_unchecked(&e4);
        let d5 = "d7d5".parse::<Uci>().expect("d5").to_move(&pos).expect("legal");
        pos.play_unchecked(&d5);
        let exd5 = "e4d5".parse::<Uci>().expect("exd5").to_move(&pos).expect("legal");
        pos.play_unchecked(&exd5);
        let qxd5 = "d8d5".parse::<Uci>().expect("Qxd5").to_move(&pos).expect("legal");
        pos.play_unchecked(&qxd5);
        let p_at_d7 = "P@d7".parse::<Uci>().expect("P@d7+").to_move(&pos).expect("legal");
        pos.play_unchecked(&p_at_d7);
        assert!(pos.is_check());
    }
}
