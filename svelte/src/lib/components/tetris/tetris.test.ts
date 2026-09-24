import { describe, expect, it } from 'vitest';

import { createGame, hardDrop, move, rotate, step } from './tetris';

describe('tetris', () => {
  it('spawns a piece and stops it at the board edges', () => {
    let game = createGame(() => 0);

    expect(game.active.kind).toBe('I');
    for (let i = 0; i < 20; i++) game = move(game, -1, 0);
    expect(game.active.x).toBe(0);
  });

  it('uses the requested viewport board size', () => {
    let game = createGame(() => 0, 16, 12);

    expect(game.board).toHaveLength(12);
    expect(game.board[0]).toHaveLength(16);
    expect(game.active.x).toBe(6);
    game = hardDrop(game, () => 0);
    expect(game.board[11].filter(Boolean)).toHaveLength(4);
  });

  it('rotates a piece when the new position is free', () => {
    let game = createGame(() => 2 / 7);

    expect(rotate(game).active.cells).not.toEqual(game.active.cells);
  });

  it('locks a dropped piece and spawns the next one', () => {
    let game = createGame(() => 0);
    game = hardDrop(game, () => 1 / 7);

    expect(game.board[19].filter(Boolean)).toHaveLength(4);
    expect(game.active.kind).toBe('I');
    expect(game.next).toBe('O');
  });

  it('clears completed lines and updates the score', () => {
    let game = createGame(() => 1 / 7);
    game.board[19] = ['J', 'J', 'J', 'J', null, null, 'J', 'J', 'J', 'J'];
    game.active.x = 4;
    game = hardDrop(game, () => 0);

    expect(game.lines).toBe(1);
    expect(game.score).toBe(100);
    expect(game.board[19].filter(Boolean)).toHaveLength(2);
  });

  it('ends the game when a new piece cannot spawn', () => {
    let game = createGame(() => 0);
    game.board[0][3] = 'J';
    game = step(game, () => 0);

    expect(game.over).toBe(false);
    game = hardDrop(game, () => 0);
    expect(game.over).toBe(true);
  });
});
