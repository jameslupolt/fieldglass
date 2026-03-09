import type { Settings } from "../types/models";

interface Props {
  settings: Settings;
  onUpdate: (settings: Settings) => void;
}

const FILTERS: {
  key: keyof Pick<
    Settings,
    | "research_grade_only"
    | "licensed_only"
    | "exclude_dead"
    | "exclude_non_organism"
  >;
  label: string;
  description: string;
}[] = [
  {
    key: "research_grade_only",
    label: "Research Grade Only",
    description: "Only show community-verified observations",
  },
  {
    key: "licensed_only",
    label: "Licensed Only",
    description: "Only show photos with Creative Commons licenses",
  },
  {
    key: "exclude_dead",
    label: "Exclude Dead",
    description: "Hide observations annotated as dead organisms",
  },
  {
    key: "exclude_non_organism",
    label: "Exclude Non-Organism",
    description: "Hide scat, tracks, bones, and other evidence",
  },
];

export default function ContentFilters({ settings, onUpdate }: Props) {
  function toggle(key: (typeof FILTERS)[number]["key"]) {
    onUpdate({ ...settings, [key]: !settings[key] });
  }

  return (
    <section className="border-b border-gray-800 pb-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gray-400">
        Content Filters
      </h2>

      <div className="space-y-3">
        {FILTERS.map((filter) => (
          <label
            key={filter.key}
            className="flex cursor-pointer items-center justify-between"
          >
            <div>
              <p className="text-sm text-gray-200">{filter.label}</p>
              <p className="text-xs text-gray-500">{filter.description}</p>
            </div>
            <button
              role="switch"
              aria-checked={settings[filter.key]}
              onClick={() => toggle(filter.key)}
              className={`relative h-6 w-11 rounded-full transition-colors ${
                settings[filter.key] ? "bg-green-600" : "bg-gray-700"
              }`}
            >
              <span
                className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
                  settings[filter.key] ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </label>
        ))}
      </div>
    </section>
  );
}
