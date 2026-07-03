<script>
  import GlassPanel from '../shared/GlassPanel.svelte';

  // Placeholder presentation content (spec D1: minimal-polished, swappable later)
  const slides = [
    { title: 'SBS N센터', body: '터치 프레젠테이션 데모', hint: '옆으로 넘겨보세요 →' },
    { title: '오늘의 아이템', body: '카드를 터치하면 반응합니다', cards: ['아이템 1', '아이템 2', '아이템 3'] },
    { title: 'XR 그래픽', body: '화면 위 팔레트 버튼을 누르면\nXR 장비에서 그래픽이 재생됩니다', hint: '↑ floating 팔레트' },
  ];
  let tapped = $state(null);

  // Drag-to-scroll: kiosk touchscreen. Finger-swipe uses native scrolling
  // (momentum + scroll-snap stay native); mouse/pen drag would otherwise
  // select text, so we pan the container manually for those pointer types.
  let mainEl = $state(null);
  let dragging = $state(false);
  let startX = 0;
  let startY = 0;
  let startScrollLeft = 0;
  let startScrollTop = 0;

  function onPointerDown(event) {
    // Leave touch to native scrolling so momentum + scroll-snap are preserved.
    if (event.pointerType === 'touch') return;
    dragging = true;
    startX = event.clientX;
    startY = event.clientY;
    startScrollLeft = mainEl.scrollLeft;
    startScrollTop = mainEl.scrollTop;
    mainEl.setPointerCapture(event.pointerId);
  }

  function onPointerMove(event) {
    if (!dragging) return;
    // Pan both axes; content is primarily horizontal, vertical is harmless.
    mainEl.scrollLeft = startScrollLeft - (event.clientX - startX);
    mainEl.scrollTop = startScrollTop - (event.clientY - startY);
  }

  function endDrag(event) {
    if (!dragging) return;
    dragging = false;
    // scroll-snap-type: x mandatory snaps to the nearest slide on release.
    if (mainEl.hasPointerCapture?.(event.pointerId)) {
      mainEl.releasePointerCapture(event.pointerId);
    }
  }
</script>

<main
  bind:this={mainEl}
  class:grabbing={dragging}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={endDrag}
  onpointercancel={endDrag}
>
  {#each slides as slide, i}
    <section>
      <GlassPanel>
        <div class="slide">
          <h1>{slide.title}</h1>
          <p>{slide.body}</p>
          {#if slide.cards}
            <div class="cards">
              {#each slide.cards as card, j}
                <button
                  class="card"
                  class:tapped={tapped === `${i}-${j}`}
                  onclick={() => (tapped = `${i}-${j}`)}
                >
                  {card}
                </button>
              {/each}
            </div>
          {/if}
          {#if slide.hint}<span class="hint">{slide.hint}</span>{/if}
        </div>
      </GlassPanel>
    </section>
  {/each}
</main>

<style>
  :global(body) {
    background:
      radial-gradient(1200px 600px at 20% 10%, #1b3a5c 0%, transparent 60%),
      radial-gradient(1000px 700px at 80% 90%, #23504a 0%, transparent 60%),
      #0d1117;
  }
  main {
    display: flex;
    height: 100vh;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }
  main.grabbing { cursor: grabbing; }
  section {
    flex: 0 0 100vw;
    scroll-snap-align: center;
    display: grid;
    place-items: center;
  }
  .slide {
    padding: 56px 72px;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 18px;
    align-items: center;
  }
  h1 { margin: 0; font-size: 44px; color: var(--text); }
  p { margin: 0; font-size: 20px; color: var(--text-dim); white-space: pre-line; }
  .cards { display: flex; gap: 16px; margin-top: 12px; }
  .card {
    min-width: 160px;
    min-height: 100px;
    border: 1px solid var(--glass-border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.06);
    color: var(--text);
    font-size: 18px;
    cursor: pointer;
    transition: transform 0.12s ease, background 0.2s ease;
  }
  .card.tapped { background: var(--accent); transform: scale(1.05); }
  .hint { font-size: 14px; color: var(--text-dim); }
</style>
