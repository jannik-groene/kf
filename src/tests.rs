use crate::chess;
//use rand::{seq::SliceRandom, thread_rng};

#[test]
fn read_start_fen() {
    let npos1 = chess::Position::new();
    let npos2 = chess::Position::from_fen(String::from(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    ))
    .unwrap();
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn no_moves_in_checkmate() {
    let pos = chess::Position::from_fen(String::from("2Q4k/4Q3/4p3/4P3/5P2/4BK2/8/8 b - - 0 106"));
    assert!(pos.unwrap().get_moves().is_empty());
}

#[test]
fn count_moves_from_start() {
    let mut pos = chess::Position::new();
    assert!(pos.get_moves().len() == 20);
}

#[test]
fn print_board() {
    let pos = chess::Position::from_fen(String::from("2Q4k/4Q3/4p3/4P3/5P2/4BK2/8/8 b - - 0 106"))
        .unwrap();
    let board = format!("{}", pos.get_board());
    assert!(board == ". . Q . . . . k \n. . . . Q . . . \n. . . . p . . . \n. . . . P . . . \n. . . . . P . . \n. . . . B K . . \n. . . . . . . . \n. . . . . . . . \n");
}

#[test]
fn simple_en_passant() {
    let mut pos = chess::Position::from_fen(String::from(
        "rnbqkbnr/pp3ppp/8/2pPp3/5P2/8/PPPP2PP/RNBQKBNR w KQkq c6 0 4",
    ))
    .unwrap();
    let m = chess::Move {
        from: 35u8.into(),
        to: 42u8.into(),
        piece: chess::Piece::Pawn,
        typ: chess::MoveType::Enpassant,
    };
    assert!(pos.get_moves().contains(&m));
    let npos1 = pos.from_move(m);
    println!("{}", npos1.board);
    let npos2 = chess::Position::from_fen(String::from(
        "rnbqkbnr/pp3ppp/2P5/4p3/5P2/8/PPPP2PP/RNBQKBNR b KQkq - 0 4",
    ))
    .unwrap();
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn simple_castle() {
    let mut pos = chess::Position::from_fen(String::from(
        "rn1qk2r/1p2bppp/p2pbn2/4p3/4P3/1NN1BP2/PPPQ2PP/R3KB1R b KQkq - 2 9",
    ))
    .unwrap();
    let m = chess::Move {
        from: 60u8.into(),
        to: 62u8.into(),
        piece: chess::Piece::King,
        typ: chess::MoveType::Castle,
    };
    assert!(pos.get_moves().contains(&m));
    let npos1 = pos.from_move(m);
    let npos2 = chess::Position::from_fen(String::from(
        "rn1q1rk1/1p2bppp/p2pbn2/4p3/4P3/1NN1BP2/PPPQ2PP/R3KB1R w KQ - 3 10",
    ))
    .unwrap();
    println!("{}", npos1.get_board());
    assert!(*npos1.get_board() == *npos2.get_board());
}

#[test]
fn simple_pin() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/4r3/8/8/4Q3/8/8/4K3 w - - 0 1")).unwrap();
    println!("Found {} moves, expected 10.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 10);
}

#[test]
fn simple_block() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/3r1r2/8/b7/8/8/r7/2B1K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn simple_remove_attacking_knight() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/3r1r2/8/8/8/8/r5n1/4K2B w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn simple_double_block() {
    let mut pos = chess::Position::from_fen(String::from(
        "4k3/4r3/8/3p1p2/b3P3/3p1p1b/r3P2r/4K3 w - - 0 1",
    ))
    .unwrap();
    assert!(pos.get_moves().len() == 6);
}

#[test]
fn simple_pinned_no_en_passant() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/8/4r3/3pP3/3r1r2/8/r7/4K3 w - d6 0 1"))
            .unwrap();
    println!("Found {} moves, expected 0.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    println!("{}", pos.get_board());
    assert!(pos.get_moves().is_empty());
}

#[test]
fn simple_remove_attacker() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/3r1r2/8/8/8/8/r2q4/2B1K3 w - - 0 1")).unwrap();
    println!("Found {} moves, expected 1.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 1);
}

#[test]
fn move_count_test_2() {
    let mut pos = chess::Position::from_fen(String::from(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0",
    ))
    .unwrap();
    assert_eq!(pos.get_moves().len(), 48);
}

#[test]
fn simple_pinned_pawn_attack() {
    let mut pos =
        chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 2"))
            .unwrap();
    pos = pos.from_move(chess::Move {
        from: 12u8.into(),
        to: 28u8.into(),
        piece: chess::Piece::Pawn,
        typ: chess::MoveType::Normal,
    });
    println!("Found {} moves, expected 16.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 16);
}

#[test]
fn simple_avoid_check() {
    let mut pos =
        chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/5Rk1/8/4P3/8 b - - 0 2")).unwrap();
    println!("Found {} moves, expected 4.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 4);
}

#[test]
fn simple_en_passant_check_avoidance() {
    let mut pos =
        chess::Position::from_fen(String::from("5b1k/8/8/2pP4/8/K7/8/8 w - c6 0 1")).unwrap();
    println!("Found {} moves, expected 5.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 5);
    pos = chess::Position::from_fen(String::from("7k/8/8/K1pP3r/8/8/8/8 w - c6 0 1")).unwrap();
    println!("Found {} moves, expected 5.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {} from {} to {}", m.piece as u8, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 5);
}

#[test]
fn simple_en_passant_remove_attacker() {
    let mut pos =
        chess::Position::from_fen(String::from("4k3/8/8/3pP3/4K3/8/8/8 w - d6 0 1")).unwrap();
    println!("Found {} moves, expected 8.", pos.get_moves().len());
    for m in pos.get_moves() {
        println!("Move {:?} from {} to {}", m.piece, m.from, m.to);
    }
    assert!(pos.get_moves().len() == 8);
}

#[test]
fn simple_rook_capture() {
    let mut pos = chess::Position::from_fen(String::from(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N1Q3/PPPBBPpP/R3K2R b KQkq - 1 2",
    ))
    .unwrap();
    pos = pos.from_move(chess::Move {
        from: 14u8.into(),
        to: 7u8.into(),
        piece: chess::Piece::Pawn,
        typ: chess::MoveType::PromotionCapture((chess::Piece::Knight, chess::Piece::Rook)),
    });
    assert!(pos.get_moves().len() == 47);
}

#[test]
fn simple_pawn_advance() {
    let mut pos =
        chess::Position::from_fen(String::from("3rkr2/8/8/8/8/4r3/4P3/4K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().is_empty());
    pos = chess::Position::from_fen(String::from("3rkr2/8/8/8/4r3/8/4P3/4K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 1);
    pos = chess::Position::from_fen(String::from("3rkr2/8/8/4r3/8/8/4P3/4K3 w - - 0 1")).unwrap();
    assert!(pos.get_moves().len() == 2);
}

fn do_perft(pos: &mut chess::Position, depth: usize) -> usize {
    if depth == 0 {
        1
    } else {
        pos.get_moves().iter().fold(0, |sum, m| {
            sum + do_perft(&mut pos.from_move(*m), depth - 1)
        })
    }
}


#[test]
#[ignore]
fn perft_7() {
    let mut pos = chess::Position::new();
    let res = do_perft(&mut pos, 7);
    assert_eq!(res, 3_195_901_860);
//6    assert_eq!(res, 119_060_324);
}

#[test]
fn move_count_test() {
    let positions = [
        //Position 1
        //WORKS TO DEPTH 6!
        chess::Position::new(),
        //Position 2
        //WORKS TO DEPTH 6!
        chess::Position::from_fen(String::from(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 0",
        ))
        .unwrap(),
        //Position 3
        //WORKS TO DEPTH 8!
        chess::Position::from_fen(String::from("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 0"))
            .unwrap(),
        //Position 4
        //WORK TO DEPTH 6!
        chess::Position::from_fen(String::from(
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        ))
        .unwrap(),
        //Position 5
        //WORK TO DEPTH 6!
        chess::Position::from_fen(String::from(
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        ))
        .unwrap(),
        //Poistion 6
        //WORKS TO DEPTH 6!
        chess::Position::from_fen(String::from(
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        ))
        .unwrap(),
    ];
    let results: Vec<[usize; 7]> = vec![
        [1, 20, 400, 8902, 197281, 4865609, 119060324],
        [1, 48, 2039, 97862, 4085603, 193690690, 8031647685],
        [1, 14, 191, 2812, 43238, 674624, 11030083],
        [1, 6, 264, 9467, 422333, 15833292, 706045033],
        [1, 44, 1486, 62379, 2103487, 89941194, 3048196529],
        [1, 46, 2079, 89890, 3894594, 164075551, 6923051137],
    ];
    //Choose a test depth between 1 and 6, depth 6 takes about 40 minutes
    let depth: usize = 4;
    for (pos, res) in positions.iter().zip(results.iter()) {
        assert_eq!(do_perft(&mut pos.clone(), depth), res[depth]);
    }
}

//fn do_and_undo_random_moves(pos: &mut chess::Position, count: usize) {
//    let moves = pos.get_moves();
//    if !moves.is_empty() && count > 0 {
//        let m = *pos.get_moves().choose(&mut thread_rng()).unwrap();
//        let zobrist = pos.zobrist_hash();
//        pos.do_move(m);
//        do_and_undo_random_moves(pos, count - 1);
//        pos.undo_move();
//        println!("{},{:?}", m, m.typ);
//        println!("0x{:016x},0x{:016x}", zobrist, pos.zobrist_hash());
//        assert_eq!(pos.zobrist_hash(), zobrist);
//    }
//}

//#[test]
//fn undo_moves() {
//    //Undo normal move
//    let mut pos = chess::Position::new();
//    let mov1 = chess::Move {
//        from: 1u8.into(),
//        to: 16u8.into(),
//        piece: chess::Piece::Knight,
//        typ: chess::MoveType::Normal,
//    };
//    let mut pos2 = pos.from_move(mov1);
//    pos2.undo_move();
//    assert!(*pos.get_board() == *pos2.get_board());
//    //Undo capture move
//    pos = chess::Position::from_fen(String::from(
//        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
//    ))
//    .unwrap();
//    let mov2 = chess::Move {
//        from: 21u8.into(),
//        to: 45u8.into(),
//        piece: chess::Piece::Queen,
//        typ: chess::MoveType::Capture(chess::Piece::Knight),
//    };
//    pos2 = pos.from_move(mov2);
//    pos2.undo_move();
//    assert!(*pos.get_board() == *pos2.get_board());
//    //Undo enpassant
//    pos = chess::Position::from_fen(String::from(
//        "rnbqkbnr/pp3ppp/8/2pPp3/5P2/8/PPPP2PP/RNBQKBNR w KQkq c6 0 4",
//    ))
//    .unwrap();
//    let mov3 = chess::Move {
//        from: 35u8.into(),
//        to: 42u8.into(),
//        piece: chess::Piece::Pawn,
//        typ: chess::MoveType::Enpassant,
//    };
//    pos2 = pos.from_move(mov3);
//    pos2.undo_move();
//    assert!(*pos.get_board() == *pos2.get_board());
//    //Undo castling
//    pos = chess::Position::from_fen(String::from(
//        "rn1qk2r/1p2bppp/p2pbn2/4p3/4P3/1NN1BP2/PPPQ2PP/R3KB1R b KQkq - 2 9",
//    ))
//    .unwrap();
//    let mov4 = chess::Move {
//        from: 60u8.into(),
//        to: 62u8.into(),
//        piece: chess::Piece::King,
//        typ: chess::MoveType::Castle,
//    };
//    pos2 = pos.from_move(mov4);
//    pos2.undo_move();
//    assert!(*pos.get_board() == *pos2.get_board());
//    //pawn promotion and capture
//    pos = chess::Position::from_fen(String::from(
//        "rnbq1bnr/pppkpPpp/8/8/8/3p4/PPPP1PPP/RNBQKBNR w KQ - 1 5",
//    ))
//    .unwrap();
//    let mov5 = chess::Move::from_str("f7g8q", pos.get_board());
//    pos.do_move(mov5);
//    pos.undo_move();
//    //random checks
//    for _ in 0..10000 {
//        pos = chess::Position::new();
//        do_and_undo_random_moves(&mut pos, 40);
//        assert!(*pos.get_board() == chess::Board::new());
//    }
//}

//#[test]
//fn mate_in_three_test() {
//    //let pos = engine::chess::Position::from_fen(String::from("8/8/8/3k4/7R/Q7/PP5B/KP6 w - - 0 1")).unwrap();
//    //let pos = engine::chess::Position::from_fen(String::from("R7/8/8/2n5/1pN5/1p6/k2K4/4R3 b - - 1 1")).unwrap();
//    //let pos = engine::chess::Position::from_fen(String::from("5R2/8/8/2n5/1pN5/1p6/k2K4/4R3 w - - 0 1")).unwrap();
//    //let pos = engine::chess::Position::from_fen(String::from("r3kbnr/pp3ppp/8/4p3/8/6PK/P3qp1P/6R1 b kq - 1 22")).unwrap();
//    //let pos = engine::chess::Position::from_fen(String::from("3q1r1k/6Rp/5PP1/8/1P2p3/2p1P3/8/2K2R2 w - - 0 5")).unwrap();
//    let pos = engine::chess::Position::from_fen(String::from("4nr1k/p1p1p1pp/bp1pn1r1/8/6QR/6RP/1BBq1PP1/6K1 w - - 0 1")).unwrap();
//    //let pos = engine::chess::Position::new();
//    let mut stree = engine::mcts::MCSTree::new(pos);
//    stree.search_timed(Duration::from_millis(1000));
//    println!("Eval {}\nBest Move {}", stree.root_eval(), stree.best_move());
//    panic!();
//}
