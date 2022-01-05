use criterion::{black_box, criterion_group, criterion_main, Criterion};
use criterion_cycles_per_byte::CyclesPerByte;
use kf_internals::chess;

fn move_gen_in_opening(c: &mut Criterion<CyclesPerByte>) {
    let mut pos = chess::Position::from_fen(String::from(
        "r3kb1r/1pp1npp1/p1p2q2/4p2p/4P1b1/3P1N1P/PPPN1PP1/R1BQ1RK1 w kq - 3 9",
    ))
    .unwrap();
    c.bench_function("move gen in opening", |b| b.iter(|| pos.get_moves()));
}

fn attack_gen_in_opening(c: &mut Criterion<CyclesPerByte>) {
    let mut pos = chess::Position::from_fen(String::from(
        "r3kb1r/1pp1npp1/p1p2q2/4p2p/4P1b1/3P1N1P/PPPN1PP1/R1BQ1RK1 w kq - 3 9",
    ))
    .unwrap();
    c.bench_function("attack gen in opening", |b| {
        b.iter(|| pos.generate_attack_table())
    });
}

fn do_move_en_passant(c: &mut Criterion<CyclesPerByte>) {
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
    c.bench_function("do move en passant", |b| {
        b.iter(|| {
            pos.do_move(m);
            pos.undo_move();
        })
    });
}

fn perft_step(pos: &mut chess::Position, d: u8) -> usize {
    if d == 0 {
        1
    } else if d == 1 {
        pos.get_moves().len()
    } else {
        let mut total = 0;
        for m in pos.get_moves() {
            pos.do_move(m);
            total += perft_step(pos, d - 1);
            pos.undo_move();
        }
        total
    }
}

pub fn perft(c: &mut Criterion) {
    let mut pos = chess::Position::new();
    let d = 4;
    c.bench_function("perft 4", |b| {
        b.iter(|| {
            perft_step(&mut pos, black_box(d));
        })
    });
}

criterion_group!(name = move_gen_benches;
                 config = Criterion::default().with_measurement(CyclesPerByte);
                 targets = move_gen_in_opening,attack_gen_in_opening,do_move_en_passant);
criterion_group!(name = perft_bench;
                 config = Criterion::default();
                 targets = perft);
criterion_main!(move_gen_benches, perft_bench);
