import { useState, useRef, useEffect } from "react";
import type { Settings, SearchRadius, GeocodingResult } from "../types/models";
import { searchLocation } from "../lib/commands";

const RADIUS_OPTIONS: { value: SearchRadius; label: string }[] = [
  { value: "Km10", label: "10 km" },
  { value: "Km25", label: "25 km" },
  { value: "Km50", label: "50 km" },
  { value: "Km100", label: "100 km" },
];

interface Props {
  settings: Settings;
  onUpdate: (settings: Settings) => void;
}

export default function LocationSettings({ settings, onUpdate }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<GeocodingResult[]>([]);
  const [showDropdown, setShowDropdown] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (query.trim().length < 2) {
      setResults([]);
      setShowDropdown(false);
      return;
    }

    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        const res = await searchLocation(query);
        setResults(res);
        setShowDropdown(res.length > 0);
      } catch {
        setResults([]);
      }
    }, 300);

    return () => clearTimeout(debounceRef.current);
  }, [query]);

  function selectLocation(result: GeocodingResult) {
    onUpdate({
      ...settings,
      location: {
        lat: result.lat,
        lng: result.lng,
        display_name: result.display_name,
      },
    });
    setQuery("");
    setShowDropdown(false);
  }

  return (
    <section className="border-b border-gray-800 pb-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gray-400">
        Location
      </h2>

      <div className="relative">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search for a city or place..."
          className="w-full rounded-lg border border-gray-700 bg-gray-800 px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:border-green-500 focus:outline-none"
        />

        {showDropdown && (
          <ul className="absolute z-10 mt-1 max-h-48 w-full overflow-y-auto rounded-lg border border-gray-700 bg-gray-800 shadow-lg">
            {results.map((r, i) => (
              <li key={i}>
                <button
                  onClick={() => selectLocation(r)}
                  className="w-full px-3 py-2 text-left text-sm text-gray-200 hover:bg-gray-700"
                >
                  {r.display_name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {settings.location && (
        <div className="mt-3 text-sm text-gray-400">
          <p className="text-gray-200">
            {settings.location.display_name ?? "Custom location"}
          </p>
          <p className="mt-0.5 text-xs">
            {settings.location.lat.toFixed(4)}, {settings.location.lng.toFixed(4)}
          </p>
        </div>
      )}

      <div className="mt-3 flex gap-2">
        {RADIUS_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() =>
              onUpdate({ ...settings, search_radius: opt.value })
            }
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              settings.search_radius === opt.value
                ? "bg-green-600 text-white"
                : "bg-gray-800 text-gray-400 hover:bg-gray-700"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>
    </section>
  );
}
