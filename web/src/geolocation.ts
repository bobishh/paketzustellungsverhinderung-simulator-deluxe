export type Neighbourhood = { lat: number; lon: number };

/** Only the neighbourhood leaves this callback, never the browser's precise fix. */
export function requestNeighbourhood(geolocation?: Pick<Geolocation, 'getCurrentPosition'>): Promise<Neighbourhood> {
  if (!geolocation) return Promise.reject(new Error('Location is unavailable in this browser. Search for an address or choose a city.'));
  return new Promise((resolve, reject) => {
    geolocation.getCurrentPosition(position => {
      const { latitude, longitude } = position.coords;
      if (!Number.isFinite(latitude) || !Number.isFinite(longitude) || Math.abs(latitude) > 85 || Math.abs(longitude) > 180) {
        reject(new Error('This location is outside the supported map. Search for an address or choose a city.'));
        return;
      }
      // A fixed 0.01° grid (~1.1 km north–south). No reverse geocoding, tracking,
      // storage, URL update or analytics. Only this coarse origin enters the game.
      resolve({ lat: Number(latitude.toFixed(2)), lon: Number(longitude.toFixed(2)) });
    }, error => {
      const message = error.code === 1 ? 'Location permission was denied. Allow it in your browser’s site settings, or search for an address.'
        : error.code === 3 ? 'Finding your location took too long. Try again or search for an address.'
          : 'Couldn’t find your location. Try again or search for an address.';
      reject(new Error(message));
    }, { enableHighAccuracy: false, timeout: 12000, maximumAge: 60000 });
  });
}
