export const WIDTH = 10;
export const HEIGHT = 20;

const SHAPES = {
  I: [[1, 1, 1, 1]],
  O: [
    [1, 1],
    [1, 1],
  ],
  T: [
    [0, 1, 0],
    [1, 1, 1],
  ],
  S: [
    [0, 1, 1],
    [1, 1, 0],
  ],
  Z: [
    [1, 1, 0],
    [0, 1, 1],
  ],
  J: [
    [1, 0, 0],
    [1, 1, 1],
  ],
  L: [
    [0, 0, 1],
    [1, 1, 1],
  ],
} as const;

/** The seven crate shapes used on the board. */
export type Kind = keyof typeof SHAPES;
/** A board square is empty or contains a crate from one shape. */
export type Cell = Kind | null;

/** A falling shape and its current board position. */
export interface Piece {
  kind: Kind;
  cells: number[][];
  x: number;
  y: number;
}

/** All state needed to resume a game between timer ticks. */
export interface Game {
  board: Cell[][];
  active: Piece;
  next: Kind;
  score: number;
  lines: number;
  over: boolean;
}

const KINDS = Object.keys(SHAPES) as Kind[];

function emptyRow(width: number): Cell[] {
  return Array.from({ length: width }, () => null);
}

function randomKind(random: () => number): Kind {
  return KINDS[Math.floor(random() * KINDS.length)];
}

function spawn(kind: Kind, width: number): Piece {
  let cells = SHAPES[kind].map(row => [...row]);
  return { kind, cells, x: Math.floor((width - cells[0].length) / 2), y: 0 };
}

function fits(board: Cell[][], piece: Piece): boolean {
  return piece.cells.every((row, dy) =>
    row.every((cell, dx) => {
      if (!cell) return true;
      let x = piece.x + dx;
      let y = piece.y + dy;
      return x >= 0 && x < board[0].length && y >= 0 && y < board.length && board[y][x] === null;
    }),
  );
}

/** Start with an empty board of the requested size and two randomly selected shapes. */
export function createGame(random = Math.random, width = WIDTH, height = HEIGHT): Game {
  return {
    board: Array.from({ length: height }, () => emptyRow(width)),
    active: spawn(randomKind(random), width),
    next: randomKind(random),
    score: 0,
    lines: 0,
    over: false,
  };
}

/** Move a shape if its destination is free. */
export function move(game: Game, dx: number, dy: number): Game {
  if (game.over) return game;
  let active = { ...game.active, x: game.active.x + dx, y: game.active.y + dy };
  return fits(game.board, active) ? { ...game, active } : game;
}

/** Rotate clockwise, allowing one square of clearance at either wall. */
export function rotate(game: Game): Game {
  if (game.over) return game;
  let cells = game.active.cells[0].map((_, x) => game.active.cells.map(row => row[x]).toReversed());
  for (let offset of [0, -1, 1]) {
    let active = { ...game.active, cells, x: game.active.x + offset };
    if (fits(game.board, active)) return { ...game, active };
  }
  return game;
}

function lock(game: Game, random: () => number): Game {
  let board = game.board.map(row => [...row]);
  for (let [dy, row] of game.active.cells.entries()) {
    for (let [dx, cell] of row.entries()) {
      if (cell) board[game.active.y + dy][game.active.x + dx] = game.active.kind;
    }
  }

  let remaining = board.filter(row => row.includes(null));
  let cleared = board.length - remaining.length;
  board = [...Array.from({ length: cleared }, () => emptyRow(board[0].length)), ...remaining];

  let active = spawn(game.next, board[0].length);
  return {
    board,
    active,
    next: randomKind(random),
    score: game.score + cleared * cleared * 100,
    lines: game.lines + cleared,
    over: !fits(board, active),
  };
}

/** Advance the falling shape or lock it when it reaches an obstacle. */
export function step(game: Game, random = Math.random): Game {
  if (game.over) return game;
  let moved = move(game, 0, 1);
  return moved === game ? lock(game, random) : moved;
}

/** Drop the current shape as far as possible, then lock it. */
export function hardDrop(game: Game, random = Math.random): Game {
  if (game.over) return game;
  let moved = game;
  while (true) {
    let next = move(moved, 0, 1);
    if (next === moved) return lock(moved, random);
    moved = next;
  }
}
