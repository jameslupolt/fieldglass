import type { Settings, AspectRatioMode } from "../types/models";

interface Props {
  settings: Settings;
  onUpdate: (settings: Settings) => void;
}

export default function DisplaySettings({ settings, onUpdate }: Props) {
  function setDuration(value: number) {
    onUpdate({ ...settings, photo_duration_secs: value });
  }

  function setAspectRatio(mode: AspectRatioMode) {
    onUpdate({ ...settings, aspect_ratio_mode: mode });
  }

  function setOverlayOpacity(value: number) {
    onUpdate({ ...settings, overlay_opacity: value / 100 });
  }

  return (
    <section className="border-b border-gray-800 pb-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gray-400">
        Display
      </h2>

      <div className="space-y-4">
        <div>
          <label className="mb-1 block text-sm text-gray-300">
            Photo Duration: {settings.photo_duration_secs}s
          </label>
          <input
            type="range"
            min={5}
            max={60}
            step={5}
            value={settings.photo_duration_secs}
            onChange={(e) => setDuration(Number(e.target.value))}
            className="w-full accent-green-500"
          />
          <div className="mt-1 flex justify-between text-xs text-gray-600">
            <span>5s</span>
            <span>60s</span>
          </div>
        </div>

        <div>
          <label className="mb-2 block text-sm text-gray-300">
            Aspect Ratio
          </label>
          <div className="flex gap-2">
            <button
              onClick={() => setAspectRatio("contain")}
              className={`flex-1 rounded-lg px-3 py-2 text-sm transition-colors ${
                settings.aspect_ratio_mode === "contain"
                  ? "bg-green-600 text-white"
                  : "bg-gray-800 text-gray-400 hover:bg-gray-700"
              }`}
            >
              <span className="font-medium">Contain</span>
              <p className="mt-0.5 text-xs opacity-75">
                Fit whole photo with black bars
              </p>
            </button>
            <button
              onClick={() => setAspectRatio("fill")}
              className={`flex-1 rounded-lg px-3 py-2 text-sm transition-colors ${
                settings.aspect_ratio_mode === "fill"
                  ? "bg-green-600 text-white"
                  : "bg-gray-800 text-gray-400 hover:bg-gray-700"
              }`}
            >
              <span className="font-medium">Fill</span>
              <p className="mt-0.5 text-xs opacity-75">
                Crop to fill screen (ND photos stay letterboxed)
              </p>
            </button>
          </div>
        </div>

        <div>
          <label className="mb-1 block text-sm text-gray-300">
            Overlay Opacity: {Math.round(settings.overlay_opacity * 100)}%
          </label>
          <input
            type="range"
            min={0}
            max={100}
            value={Math.round(settings.overlay_opacity * 100)}
            onChange={(e) => setOverlayOpacity(Number(e.target.value))}
            className="w-full accent-green-500"
          />
          <div className="mt-1 flex justify-between text-xs text-gray-600">
            <span>Hidden</span>
            <span>Opaque</span>
          </div>
        </div>
      </div>
    </section>
  );
}
