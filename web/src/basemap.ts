import type {Map as VectorMap} from 'maplibre-gl';
import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';

type Camera = {lat: number; lon: number; zoom: number};

const container = document.querySelector<HTMLElement>('#lobby-map')!;
const credit = document.querySelector<HTMLElement>('#lobby-map-credit')!;
declare global { var paketLobbyMapSize: () => [number, number]; }
globalThis.paketLobbyMapSize = () => [container.clientWidth, container.clientHeight];

let view: {camera: Camera; map?: VectorMap; timeout?: number; raster?: boolean} | undefined;

function mapCamera({lat, lon, zoom}: Camera) {
  // Bevy measures zoom against 256 px tiles; MapLibre uses 512 px.
  return {center: [lon, lat] as [number, number], zoom: zoom - 1};
}

async function openMap(session: NonNullable<typeof view>) {
  const useRaster = () => {
    if (view !== session || session.raster) return;
    session.raster = true;
    removeMap(session);
    globalThis.paketCommands.push({action: 'raster-map'});
  };
  try {
    session.timeout = window.setTimeout(useRaster, 12_000);
    const {Map, setWorkerUrl} = await import('maplibre-gl');
    if (view !== session || session.raster) return;
    setWorkerUrl(workerUrl);
    const map = new Map({
      container,
      ...mapCamera(session.camera),
      style: 'https://tiles.openfreemap.org/styles/fiord',
      interactive: false,
      attributionControl: false,
      fadeDuration: 0,
    });
    session.map = map;
    map.once('load', () => {
      window.clearTimeout(session.timeout);
      if (map.getLayer('highway_name_other')) {
        map.setPaintProperty('highway_name_other', 'text-color', '#e5edf5');
      }
      container.dataset.ready = 'true';
      credit.hidden = false;
      window.dispatchEvent(new CustomEvent('paket-map-loading', {
        detail: {ready: true, pending: false, failed: false},
      }));
    });
    map.on('error', useRaster);
  } catch {
    useRaster();
  }
}

function removeMap(session: NonNullable<typeof view>) {
  window.clearTimeout(session.timeout);
  delete container.dataset.ready;
  credit.hidden = true;
  session.map?.remove();
  session.map = undefined;
}

window.addEventListener('paket-mode', event => {
  if ((event as CustomEvent<string>).detail !== 'map' && view) {
    const session = view;
    view = undefined;
    removeMap(session);
  }
});
window.addEventListener('paket-lobby', event => {
  const {lat, lon, zoom} = (event as CustomEvent<Camera>).detail;
  if (!Number.isFinite(zoom)) return;
  if (!view) {
    view = {camera: {lat, lon, zoom}};
    void openMap(view);
  } else {
    const {camera, map} = view;
    if (lat === camera.lat && lon === camera.lon && zoom === camera.zoom) return;
    view.camera = {lat, lon, zoom};
    map?.jumpTo(mapCamera(view.camera));
  }
});
