use super::*;
use itertools::Itertools;

#[test]
fn read_start_fen() {
    let npos1 = engine::chess::Position::new();
    let npos2 = engine::chess::Position::from_fen(String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")).unwrap();
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn no_moves_in_checkmate() {
    let pos = engine::chess::Position::from_fen(String::from("2Q4k/4Q3/4p3/4P3/5P2/4BK2/8/8 b - - 0 106"));
    assert!(pos.unwrap().get_moves().len() == 0);
}

#[test]
fn count_moves_from_start() {
    let mut pos = engine::chess::Position::new();
    assert!(pos.get_moves().len()==20);
}

#[test]
fn print_board() {
    let pos = engine::chess::Position::from_fen(String::from("2Q4k/4Q3/4p3/4P3/5P2/4BK2/8/8 b - - 0 106")).unwrap();
    let board = format!("{}", pos.get_board());
    assert!(board == ". . Q . . . . k \n. . . . Q . . . \n. . . . p . . . \n. . . . P . . . \n. . . . . P . . \n. . . . B K . . \n. . . . . . . . \n. . . . . . . . \n");
}

#[test]
fn simple_en_passant() {
    let mut pos = engine::chess::Position::from_fen(String::from("rnbqkbnr/pp3ppp/8/2pPp3/5P2/8/PPPP2PP/RNBQKBNR w KQkq c6 0 4")).unwrap();
    let m = engine::chess::Move{
        from: 1 << 35,
        to: 1 << 42,
        piece: engine::chess::Piece::PAWN,
        promote: None,
    };
    assert!(pos.get_moves().contains(&m));
    let npos1 = pos.do_move(m);
    let npos2 = engine::chess::Position::from_fen(String::from("rnbqkbnr/pp3ppp/2P5/4p3/5P2/8/PPPP2PP/RNBQKBNR b KQkq - 0 4")).unwrap();
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn simple_castle() {
    let mut pos = engine::chess::Position::from_fen(String::from("rn1qk2r/1p2bppp/p2pbn2/4p3/4P3/1NN1BP2/PPPQ2PP/R3KB1R b KQkq - 2 9")).unwrap();
    let m = engine::chess::Move {
        from: 1 << 60,
        to: 1 << 62,
        piece: engine::chess::Piece::KING,
        promote: None,
    };
    assert!(pos.get_moves().contains(&m));
    let npos1 = pos.do_move(m);
    let npos2 = engine::chess::Position::from_fen(String::from("rn1q1rk1/1p2bppp/p2pbn2/4p3/4P3/1NN1BP2/PPPQ2PP/R3KB1R w KQ - 3 10")).unwrap();
    println!("{}", npos1.get_board());
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn simple_pin() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/4r3/8/8/4Q3/8/8/4K3 w - - 0 1")).unwrap();
    println!("Found {} moves, expected 10.",pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 10);
}

#[test]
fn simple_block() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/3r1r2/8/b7/8/8/r7/2B1K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn simple_remove_attacking_knight() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/3r1r2/8/8/8/8/r5n1/4K2B w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn simple_double_block() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/4r3/8/3p1p2/b3P3/3p1p1b/r3P2r/4K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 6);
}


#[test]
fn simple_pinned_no_en_passant() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/8/4r3/3pP3/3r1r2/8/r7/4K3 w - d6 0 1")).unwrap();
    println!("Found {} moves, expected 0.",pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    println!("{}", pos.get_board());
    assert!(pos.get_moves().len() == 0);
}

#[test]
fn simple_remove_attacker() {
    let mut pos = engine::chess::Position::from_fen(String::from("4k3/3r1r2/8/8/8/8/r2q4/2B1K3 w - - 0 1")).unwrap();
    println!("Found {} moves, expected 1.",pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn move_count_test_2() {
    let mut pos = engine::chess::Position::from_fen(String::from("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0")).unwrap();
    assert!(pos.get_moves().len()==48);
}

#[test]
fn simple_pinned_pawn_attack() {
    let mut pos = engine::chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 2")).unwrap();
    pos = pos.do_move(engine::chess::Move {
        from: 1 << 12,
        to: 1 << 28,
        piece: engine::chess::Piece::PAWN,
        promote: None,
    });
    println!("Found {} moves, expected 16.",pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 16);
}

#[test]
fn simple_avoid_check() {
    let mut pos = engine::chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/5Rk1/8/4P3/8 b - - 0 2")).unwrap();
    println!("Found {} moves, expected 4.",pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 4);
}

#[test]
fn move_count_test() {
    let mut positions =
        //vec![engine::chess::Position::from_fen(String::from("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0")).unwrap()];
        vec![engine::chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 0")).unwrap()];
        //vec![engine::chess::Position::new()];
    let mut new_positions = Vec::new();
    const COUNT: usize = 5;
    let mut pos_counts = [0; COUNT];
    for i in 0..COUNT {
        for p in positions.iter_mut() {
            let moves = p.get_moves();
            for m in moves {
                let np = p.do_move(m);
                new_positions.push(np);
            }
        }
        positions = new_positions;
        new_positions = Vec::new();
        pos_counts[i] = positions.len();
    }
    println!("{:?}", pos_counts);
    panic!("POLLY WANT OUTPUT");
}
