# kf Chess Engine

# Running

Download the project with

```bash 
    $ git clone https://github.com/jannik-groene/kf.git
```

Download the current net [here](https://github.com/jannik-groene/kf_nets/blob/main/f3f2fd88.nnue) and place it in the top-level directory, next to this Readme. Then build with

```bash
    $ cargo build --release
```

or

```bash
    $ RUSTFLAGS="-C target-cpu=native" cargo build --release
```

The resulting binary can be used with any chess interface supporting the UCI protocol ([external link](https://backscattering.de/chess/uci/)), such as, for example, [cutechess](https://github.com/cutechess/cutechess) or [enCroissant](https://github.com/franciscoBSalgueiro/en-croissant). It can also be run in the terminal, using the same protocol by hand.

# Features

## Evaluation

Evaluation is performed with a self-trained NNUE with a simple `(768->128)x2->1x4` architecture. Training data was generated using [kf_datagen](https://github.com/jannik-groene/kf_datagen), and consists entirely of self play data, starting from the HCE of `kf-0.0.9`. [bullet](https://github.com/jw1912/bullet) was used for training.

## Search

`kf` uses alpha-beta search, and implements many of the standard search techniques, such as

- Principal Variation Search
- Iterative Deepening
- Aspiration Windows
- Transposition Table
- Quiescence Search
- Razoring
- Reverse Futility Pruning
- Singular Extensions
- Null Move Pruning
- Late Move Pruning
- Futility Pruning
- Late Move Reductions
- Killer Heuristic
- Butterfly History
- Continuation History
- Capture History
- Static Evaluation Correction History
- LazySMP

## Playing strength

Testing suggests a rating around 3100 in CCRL Blitz conditions.

## A note on portability

In its current state `kf` uses the `pdep`/`pext` instructions explicitely for move generation, with no fallback. Any CPU without the BMI2 extension of the x86-64 instruction set is thus not supported. Any modern (last ~10 years) x86 CPU will likely support these instructions. Note however that early AMD Zen architectures implement these instructions via slow microcode fallbacks. Expect severe performance penalties if you are using one of these.

# Credits

Much of the information I used in creating comes from the [Chess Programming Wiki](https://chessprogramming.org/). Further, the work of other open source engines has heavily influenced the development, mostly [Stockfish](https://github.com/official-stockfish/Stockfish) and [Reckless](https://github.com/codedeliveryservice/Reckless). Lastly I have used [bullet](https://github.com/jw1912/bullet) to train all of `kf`'s networks. I would like to thank everyone involved in these projects in particular, and in the wider open source chess programming community in general.

Everything in this and the related repositories is hand-written.
