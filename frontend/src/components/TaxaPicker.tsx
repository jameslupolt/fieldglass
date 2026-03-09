import { useState, useRef, useEffect } from "react";
import type { Settings, Taxon } from "../types/models";
import { searchTaxa } from "../lib/commands";

const PRESETS: { label: string; id: number | null }[] = [
  { label: "All Life", id: null },
  { label: "Plants", id: 47126 },
  { label: "Animals", id: 1 },
  { label: "Fungi", id: 47170 },
  { label: "Birds", id: 3 },
  { label: "Insects", id: 47158 },
];

interface Props {
  settings: Settings;
  onUpdate: (settings: Settings) => void;
}

export default function TaxaPicker({ settings, onUpdate }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Taxon[]>([]);
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
        const res = await searchTaxa(query);
        setResults(res);
        setShowDropdown(res.length > 0);
      } catch {
        setResults([]);
      }
    }, 300);

    return () => clearTimeout(debounceRef.current);
  }, [query]);

  function addTaxon(taxon: Taxon) {
    if (settings.taxon_ids.includes(taxon.id)) return;
    onUpdate({ ...settings, taxon_ids: [...settings.taxon_ids, taxon.id] });
    setQuery("");
    setShowDropdown(false);
  }

  function removeTaxon(id: number) {
    onUpdate({
      ...settings,
      taxon_ids: settings.taxon_ids.filter((t) => t !== id),
    });
  }

  function selectPreset(id: number | null) {
    if (id === null) {
      onUpdate({ ...settings, taxon_ids: [] });
    } else {
      onUpdate({ ...settings, taxon_ids: [id] });
    }
  }

  return (
    <section className="border-b border-gray-800 pb-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gray-400">
        Taxa
      </h2>

      <div className="mb-3 flex flex-wrap gap-2">
        {PRESETS.map((preset) => (
          <button
            key={preset.label}
            onClick={() => selectPreset(preset.id)}
            className={`rounded-lg px-3 py-1.5 text-xs font-medium transition-colors ${
              (preset.id === null && settings.taxon_ids.length === 0) ||
              (preset.id !== null &&
                settings.taxon_ids.length === 1 &&
                settings.taxon_ids[0] === preset.id)
                ? "bg-green-600 text-white"
                : "bg-gray-800 text-gray-400 hover:bg-gray-700"
            }`}
          >
            {preset.label}
          </button>
        ))}
      </div>

      <div className="relative">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search for a taxon..."
          className="w-full rounded-lg border border-gray-700 bg-gray-800 px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:border-green-500 focus:outline-none"
        />

        {showDropdown && (
          <ul className="absolute z-10 mt-1 max-h-48 w-full overflow-y-auto rounded-lg border border-gray-700 bg-gray-800 shadow-lg">
            {results.map((taxon) => (
              <li key={taxon.id}>
                <button
                  onClick={() => addTaxon(taxon)}
                  className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-gray-700"
                >
                  {taxon.default_photo?.square_url && (
                    <img
                      src={taxon.default_photo.square_url}
                      alt=""
                      className="h-8 w-8 rounded object-cover"
                    />
                  )}
                  <div className="min-w-0">
                    <p className="truncate text-sm text-gray-200">
                      {taxon.preferred_common_name ?? taxon.name}
                    </p>
                    <p className="truncate text-xs italic text-gray-500">
                      {taxon.name}
                    </p>
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {settings.taxon_ids.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {settings.taxon_ids.map((id) => (
            <span
              key={id}
              className="inline-flex items-center gap-1 rounded-full bg-gray-800 px-3 py-1 text-xs text-gray-300"
            >
              #{id}
              <button
                onClick={() => removeTaxon(id)}
                className="ml-1 text-gray-500 hover:text-red-400"
              >
                &times;
              </button>
            </span>
          ))}
        </div>
      )}
    </section>
  );
}
