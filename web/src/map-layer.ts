import type {Map as VectorMap} from 'maplibre-gl';
import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';

type Viewport = {x: number; y: number; width: number; height: number; lat: number; lon: number; zoom: number};
const style = 'https://tiles.openfreemap.org/styles/fiord';

export function installMapLayer(setVector: (active: boolean) => void) {
  const layer = document.createElement('div');
  layer.id = 'lobby-map';
  layer.setAttribute('aria-hidden', 'true');
  document.querySelector('#game')!.before(layer);

  let viewport: Viewport | null = null;
  let map: VectorMap | undefined;
  let generation = 0;
  let attempted = false;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  let positioned: Viewport | undefined;

  function remove() {
    generation++;
    clearTimeout(timeout);
    delete layer.dataset.ready;
    map?.remove();
    map = undefined;
    positioned = undefined;
  }

  function position(view: Viewport) {
    if (view.x !== positioned?.x || view.y !== positioned.y ||
        view.width !== positioned.width || view.height !== positioned.height) {
      Object.assign(layer.style, {
        left: `${view.x}px`, top: `${view.y}px`,
        width: `${view.width}px`, height: `${view.height}px`,
      });
      map?.resize();
    }
    if (view.lat !== positioned?.lat || view.lon !== positioned.lon || view.zoom !== positioned.zoom) {
      // Bevy uses 256-pixel tiles and MapLibre uses a 512-pixel tile grid.
      map?.jumpTo({center: [view.lon, view.lat], zoom: view.zoom - 1});
    }
    positioned = view;
  }

  async function open() {
    attempted = true;
    const current = ++generation;
    const fallback = () => {
      if (current !== generation) return;
      remove();
      setVector(false);
    };
    timeout = setTimeout(fallback, 12_000);
    try {
      const {Map, setWorkerUrl} = await import('maplibre-gl');
      if (current !== generation || !viewport) return;
      positioned = undefined;
      position(viewport);
      setWorkerUrl(workerUrl);
      map = new Map({
        container: layer, style, center: [viewport.lon, viewport.lat], zoom: viewport.zoom - 1,
        interactive: false, attributionControl: false, dragRotate: false, fadeDuration: 0,
      });
      map.on('error', () => {
        if (!layer.hasAttribute('data-ready')) fallback();
      });
      map.once('load', () => {
        if (current !== generation) return;
        clearTimeout(timeout);
        layer.dataset.ready = '';
        setVector(true);
      });
      map.getCanvas().addEventListener('webglcontextlost', fallback, {once: true});
    } catch {
      fallback();
    }
  }

  window.addEventListener('paket-map-viewport', event => {
    viewport = (event as CustomEvent<Viewport | null>).detail;
    if (!viewport) {
      remove();
      attempted = false;
      setVector(false);
    } else if (!attempted) {
      void open();
    } else if (map) {
      position(viewport);
    }
  });
  window.addEventListener('paket-mode', event => {
    if ((event as CustomEvent<string>).detail === 'map') return;
    viewport = null;
    remove();
    attempted = false;
  });
}
