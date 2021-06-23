use criterion::{black_box, criterion_group, criterion_main, Criterion};
use criterion_cycles_per_byte::CyclesPerByte;
use kf_internals::engine;

fn move_gen_in_opening(c: &mut Criterion<CyclesPerByte>) {
    let mut pos = engine::chess::Position::from_fen(String::from("r3kb1r/1pp1npp1/p1p2q2/4p2p/4P1b1/3P1N1P/PPPN1PP1/R1BQ1RK1 w kq - 3 9")).unwrap();
    c.bench_function("move gen in opening", |b| b.iter(|| pos.get_moves()));
}

fn attack_gen_in_opening(c: &mut Criterion<CyclesPerByte>) {
    let mut pos = engine::chess::Position::from_fen(String::from("r3kb1r/1pp1npp1/p1p2q2/4p2p/4P1b1/3P1N1P/PPPN1PP1/R1BQ1RK1 w kq - 3 9")).unwrap();
    c.bench_function("attack gen in opening", |b| b.iter(|| pos.generate_attack_table()));
}

fn do_move_en_passant(c: &mut Criterion<CyclesPerByte>) {
    let pos = engine::chess::Position::from_fen(String::from("rnbqkbnr/pp3ppp/8/2pPp3/5P2/8/PPPP2PP/RNBQKBNR w KQkq c6 0 4")).unwrap();
    let m = engine::chess::Move {
        from: 1 << 35,
        to: 1 << 42,
        piece: engine::chess::Piece::PAWN,
        promote: None,
    };
    c.bench_function("do move en passant", |b| b.iter(|| pos.do_move(m)));
}

criterion_group!(name = benches;
                 config = Criterion::default().with_measurement(CyclesPerByte);
                 targets = move_gen_in_opening,attack_gen_in_opening,do_move_en_passant);
criterion_main!(benches);
