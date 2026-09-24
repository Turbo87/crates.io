<script lang="ts">
  import type { Game } from './tetris';

  import { onMount } from 'svelte';

  import { createGame, hardDrop, move, rotate, step } from './tetris';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let game = $state.raw<Game>(createGame(() => 0));
  let status = $state<'ready' | 'playing' | 'paused' | 'over'>('ready');

  onMount(() => {
    let timer = globalThis.setInterval(() => {
      if (status !== 'playing') return;
      game = step(game);
      if (game.over) status = 'over';
    }, 850);
    return () => globalThis.clearInterval(timer);
  });

  let squares = $derived.by(() => {
    let board = game.board.map(row => [...row]);
    if (status !== 'ready' && !game.over) {
      for (let [dy, row] of game.active.cells.entries()) {
        for (let [dx, cell] of row.entries()) {
          if (cell) board[game.active.y + dy][game.active.x + dx] = game.active.kind;
        }
      }
    }
    return board.flatMap((row, y) => row.map((kind, x) => ({ id: y * board[0].length + x, kind })));
  });

  function start() {
    let cellSize = Math.max(34, Math.min(72, Math.round(Math.min(globalThis.innerWidth, globalThis.innerHeight) / 10)));
    let width = Math.max(4, Math.ceil(globalThis.innerWidth / cellSize));
    let height = Math.max(4, Math.ceil(globalThis.innerHeight / cellSize));
    game = createGame(Math.random, width, height);
    status = 'playing';
  }

  function togglePause() {
    if (status === 'playing') status = 'paused';
    else if (status === 'paused') status = 'playing';
  }

  function play(action: 'left' | 'right' | 'down' | 'rotate' | 'drop') {
    if (status !== 'playing') return;
    switch (action) {
      case 'left':
        game = move(game, -1, 0);
        break;
      case 'right':
        game = move(game, 1, 0);
        break;
      case 'down':
        game = step(game);
        break;
      case 'rotate':
        game = rotate(game);
        break;
      case 'drop':
        game = hardDrop(game);
        break;
    }
    if (game.over) status = 'over';
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
      return;
    }
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
    if (event.target instanceof HTMLButtonElement && event.key === ' ') return;
    let actions: Record<string, 'left' | 'right' | 'down' | 'rotate' | 'drop'> = {
      ArrowLeft: 'left',
      ArrowRight: 'right',
      ArrowDown: 'down',
      ArrowUp: 'rotate',
      ' ': 'drop',
    };
    if (event.key.toLowerCase() === 'p') {
      event.preventDefault();
      togglePause();
    } else if (status === 'playing' && event.key in actions) {
      event.preventDefault();
      play(actions[event.key]);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="game-layout">
  <section class="game-panel" aria-label="Tetris game">
    <div
      class="board"
      style:--columns={game.board[0].length}
      style:--rows={game.board.length}
      role="img"
      aria-label="Tetris board, {game.lines} lines cleared"
    >
      {#each squares as square (square.id)}
        <div class={['square', square.kind && 'crate', square.kind && `kind-${square.kind}`]}></div>
      {/each}
    </div>
  </section>

  <aside class="sidebar" aria-label="Game information and controls">
    <button class="close" aria-label="Close Tetris" onclick={onClose}>×</button>
    <p class="status" aria-live="polite">
      {#if status === 'ready'}Ready to play{/if}
      {#if status === 'playing'}Crates are falling{/if}
      {#if status === 'paused'}Paused{/if}
      {#if status === 'over'}Game over! You cleared {game.lines} {game.lines === 1 ? 'line' : 'lines'}.{/if}
    </p>
    <div class="stats">
      <div><span>Score</span><strong>{game.score}</strong></div>
      <div><span>Lines</span><strong>{game.lines}</strong></div>
    </div>

    <div class="primary-controls">
      {#if status === 'ready' || status === 'over'}
        <button
          onclick={event => {
            start();
            if (event.detail) event.currentTarget.blur();
          }}
        >
          {status === 'ready' ? 'Start game' : 'Play again'}
        </button>
      {:else}
        <button
          onclick={event => {
            togglePause();
            if (event.detail) event.currentTarget.blur();
          }}
        >
          {status === 'paused' ? 'Resume' : 'Pause'}
        </button>
        <button
          onclick={event => {
            start();
            if (event.detail) event.currentTarget.blur();
          }}>New game</button
        >
      {/if}
    </div>
    <p class="instructions">← → move · ↑ rotate · ↓ faster · Space drop · P pause · Esc close</p>
  </aside>
</div>

<style>
  .game-layout {
    position: fixed;
    inset: 0;
    z-index: 1000;
    pointer-events: none;
  }

  .game-panel,
  .board {
    position: absolute;
    inset: 0;
  }

  .board {
    display: grid;
    grid-template-columns: repeat(var(--columns), minmax(0, 1fr));
    grid-template-rows: repeat(var(--rows), minmax(0, 1fr));
  }

  .square {
    position: relative;
  }

  .crate {
    background: linear-gradient(140deg, #ffffff3a, transparent 46%), var(--box);
    border: 1px solid var(--edge);
    box-shadow:
      inset 1px 1px #ffffff50,
      inset -2px -2px #0000002b;
  }

  .crate::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 40%;
    width: 20%;
    background: #f2d89a88;
    border-inline: 1px solid #75502b88;
  }

  .crate::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 20%;
    border-top: 1px solid #ffffff7a;
    box-shadow: 0 1px #5b381f88;
  }

  .kind-I {
    --box: #b9864e;
    --edge: #77502e;
  }
  .kind-O {
    --box: #d6a35e;
    --edge: #80582d;
  }
  .kind-T {
    --box: #ab805d;
    --edge: #694631;
  }
  .kind-S {
    --box: #9caa6a;
    --edge: #59683d;
  }
  .kind-Z {
    --box: #bd7e63;
    --edge: #794735;
  }
  .kind-J {
    --box: #8199a1;
    --edge: #475e66;
  }
  .kind-L {
    --box: #b4a06d;
    --edge: #6c5d37;
  }

  .sidebar {
    position: absolute;
    bottom: var(--space-m);
    right: var(--space-m);
    width: max-content;
    max-width: calc(100vw - 2 * var(--space-m));
    display: grid;
    gap: var(--space-s);
    padding: var(--space-s);
    background: light-dark(#f9f7ecee, #20231fee);
    border: 1px solid var(--gray-border);
    border-radius: 8px;
    box-shadow: 0 2px 10px #00000033;
    pointer-events: auto;
  }

  .status {
    margin: 0;
    max-width: 13rem;
    font-weight: 600;
    padding-right: var(--space-m);
  }

  .stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-xs);
  }

  .stats span {
    color: var(--main-color-light);
    font-size: 0.85rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .stats strong {
    display: block;
    font-size: 1.5rem;
  }

  button {
    min-height: 2.5rem;
    border: 1px solid var(--gray-border);
    border-radius: 5px;
    background: var(--main-bg);
    color: var(--main-color);
    cursor: pointer;
  }

  button:focus-visible {
    outline: 3px solid var(--yellow500);
    outline-offset: 2px;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .close {
    position: absolute;
    top: var(--space-2xs);
    right: var(--space-2xs);
    min-height: 1.5rem;
    border: 0;
    background: transparent;
    font-size: 1.4rem;
    line-height: 1;
  }

  .primary-controls {
    display: grid;
    grid-template-columns: repeat(2, max-content);
    gap: var(--space-2xs);
  }

  .primary-controls button {
    padding-inline: var(--space-xs);
    white-space: nowrap;
  }

  .primary-controls button:first-child {
    background: var(--green800);
    border-color: var(--green900);
    color: white;
    font-weight: 600;
  }

  .instructions {
    margin: 0;
    max-width: 13rem;
    color: var(--main-color-light);
    font-size: 0.75rem;
    line-height: 1.5;
  }

  @media (max-width: 650px) {
    .sidebar {
      bottom: var(--space-xs);
      right: var(--space-xs);
      max-width: calc(100vw - 2 * var(--space-xs));
      gap: var(--space-xs);
      padding: var(--space-xs);
    }
  }
</style>
