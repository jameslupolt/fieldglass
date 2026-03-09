import { useState, useEffect, useCallback } from "react";
import type { Settings, CacheStatus as CacheStatusType } from "../types/models";
import {
  getCacheStatus,
  refreshCache,
  clearCache,
} from "../lib/commands";

interface Props {
  settings: Settings;
  onUpdate: (settings: Settings) => void;
}

export default function CacheStatusPanel({ settings, onUpdate }: Props) {
  const [status, setStatus] = useState<CacheStatusType | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      const s = await getCacheStatus();
      setStatus(s);
    } catch {
      // Will show placeholder if status can't be loaded
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const noLocation = !settings.location;

  async function handleRefresh() {
    setRefreshing(true);
    setError(null);
    try {
      const s = await refreshCache();
      setStatus(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  async function handleClear() {
    if (!confirm("Clear all cached photos? They will be re-downloaded on the next refresh.")) {
      return;
    }
    setClearing(true);
    setError(null);
    try {
      await clearCache();
      await loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setClearing(false);
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  return (
    <section className="pb-6">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gray-400">
        Cache
      </h2>

      {status && (
        <div className="mb-4 grid grid-cols-2 gap-4">
          <div className="rounded-lg bg-gray-800 p-3">
            <p className="text-2xl font-bold text-green-400">
              {status.total_photos}
              <span className="text-sm font-normal text-gray-500">
                {" "}/ {status.required_photos}
              </span>
            </p>
            <p className="text-xs text-gray-500">Photos cached</p>
          </div>
          <div className="rounded-lg bg-gray-800 p-3">
            <p className="text-2xl font-bold text-gray-200">
              {formatBytes(status.cache_size_bytes)}
            </p>
            <p className="text-xs text-gray-500">Disk usage</p>
          </div>
        </div>
      )}

      <div className="mb-4 space-y-4">
        <div>
          <label className="mb-1 block text-sm text-gray-300">
            Cache Items: {settings.cache_max_items}
          </label>
          <input
            type="range"
            min={50}
            max={1000}
            step={25}
            value={settings.cache_max_items}
            onChange={(e) =>
              onUpdate({
                ...settings,
                cache_max_items: Number(e.target.value),
              })
            }
            className="w-full accent-green-500"
          />
          <div className="mt-1 flex justify-between text-xs text-gray-600">
            <span>50</span>
            <span>1000</span>
          </div>
        </div>

        <div>
          <label className="mb-1 block text-sm text-gray-300">
            Auto Refresh Interval: {settings.cache_refresh_interval_minutes} min
          </label>
          <input
            type="range"
            min={0}
            max={240}
            step={15}
            value={settings.cache_refresh_interval_minutes}
            onChange={(e) =>
              onUpdate({
                ...settings,
                cache_refresh_interval_minutes: Number(e.target.value),
              })
            }
            className="w-full accent-green-500"
          />
          <div className="mt-1 flex justify-between text-xs text-gray-600">
            <span>Off</span>
            <span>4 hr</span>
          </div>
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-lg border border-red-800 bg-red-900/30 px-3 py-2 text-sm text-red-300">
          {error}
        </div>
      )}

      {noLocation && (
        <p className="mb-3 text-sm text-yellow-400">
          Set a location in the settings above before refreshing.
        </p>
      )}

      <div className="flex gap-3">
        <button
          onClick={handleRefresh}
          disabled={refreshing || noLocation}
          className="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-500 disabled:opacity-50"
        >
          {refreshing ? "Refreshing..." : "Refresh Cache"}
        </button>
        <button
          onClick={handleClear}
          disabled={clearing}
          className="rounded-lg bg-gray-800 px-4 py-2 text-sm font-medium text-gray-300 transition-colors hover:bg-gray-700 disabled:opacity-50"
        >
          {clearing ? "Clearing..." : "Clear Cache"}
        </button>
      </div>
    </section>
  );
}
