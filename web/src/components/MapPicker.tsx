"use client";

import { useEffect } from "react";
import { MapContainer, TileLayer, Marker, useMapEvents, useMap } from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";

// divIcon avoids Leaflet's default marker asset paths, which break under bundlers
const pin = L.divIcon({ className: "", html: "<div style='font-size:28px;line-height:1'>📍</div>", iconSize: [28, 28], iconAnchor: [14, 28] });

type Point = { lat: number; lng: number };

function ClickHandler({ onChange }: { onChange: (p: Point) => void }) {
  useMapEvents({
    click(e) {
      onChange({ lat: e.latlng.lat, lng: e.latlng.lng });
    },
  });
  return null;
}

// MapContainer only applies `center` at mount, so an async geolocation update
// (which arrives after mount) would otherwise leave the viewport on the
// default location while the marker jumps off-screen. Recenter only when the
// point falls outside the current viewport, so in-view drags/clicks don't fight the user.
function Recenter({ point }: { point: Point }) {
  const map = useMap();
  useEffect(() => {
    const ll = L.latLng(point.lat, point.lng);
    if (!map.getBounds().contains(ll)) map.setView(ll, map.getZoom());
  }, [point, map]);
  return null;
}

export default function MapPicker({ value, onChange }: { value: Point; onChange: (p: Point) => void }) {
  return (
    <MapContainer center={[value.lat, value.lng]} zoom={13} className="h-64 w-full rounded border border-zinc-700">
      <TileLayer url="https://tile.openstreetmap.org/{z}/{x}/{y}.png" attribution="&copy; OpenStreetMap contributors" />
      <Marker
        position={[value.lat, value.lng]}
        draggable
        icon={pin}
        eventHandlers={{
          dragend: (e) => {
            const p = (e.target as L.Marker).getLatLng();
            onChange({ lat: p.lat, lng: p.lng });
          },
        }}
      />
      <ClickHandler onChange={onChange} />
      <Recenter point={value} />
    </MapContainer>
  );
}
