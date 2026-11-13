import { describe, expect, it, vi } from 'vitest';
import { requestNeighbourhood } from './geolocation';

function fix(latitude: number, longitude: number): GeolocationPosition {
  return { coords: { latitude, longitude, accuracy: 4, altitude: 5, altitudeAccuracy: 2, heading: 45, speed: 1 }, timestamp: 123 } as GeolocationPosition;
}

describe('location privacy boundary', () => {
  it.each([[52.489537, 13.390843, 52.49, 13.39], [-33.933571, 18.411253, -33.93, 18.41], [35.661613, 139.668237, 35.66, 139.67]])(
    'passes only the coarse neighbourhood into the game (%s, %s)', async (lat, lon, roundedLat, roundedLon) => {
      const getCurrentPosition = vi.fn<Geolocation['getCurrentPosition']>(success => success(fix(lat, lon)));
      expect(await requestNeighbourhood({ getCurrentPosition })).toEqual({ lat: roundedLat, lon: roundedLon });
      expect(getCurrentPosition).toHaveBeenCalledTimes(1);
      expect(getCurrentPosition.mock.calls[0][2]).toEqual({ enableHighAccuracy: false, timeout: 12000, maximumAge: 60000 });
    },
  );
  it.each([[1, 'permission was denied'], [2, 'Couldn’t find'], [3, 'too long']])('handles browser error %s without leaking provider details', async (code, message) => {
    const getCurrentPosition: Geolocation['getCurrentPosition'] = (_success, failure) => failure?.({ code, message: 'Sensitive provider detail' } as GeolocationPositionError);
    await expect(requestNeighbourhood({ getCurrentPosition })).rejects.toThrow(message);
  });
  it('handles unsupported browsers and rejects invalid or unsupported coordinates', async () => {
    await expect(requestNeighbourhood()).rejects.toThrow('unavailable');
    for (const [lat, lon] of [[NaN, 13], [90, 13], [52, Infinity], [52, 181]]) {
      await expect(requestNeighbourhood({ getCurrentPosition: success => success(fix(lat, lon)) })).rejects.toThrow('outside the supported map');
    }
  });
});
