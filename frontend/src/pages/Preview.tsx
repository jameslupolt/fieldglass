import { useEffect, useState, useRef, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCachedPhotos } from "../lib/commands";
import type { CachedPhoto } from "../types/models";

const DURATION_MS = 15_000;
const FADE_MS = 1500;

export default function Preview() {
  const [photos, setPhotos] = useState<CachedPhoto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [activeSlide, setActiveSlide] = useState<"a" | "b">("a");
  const [imageLoaded, setImageLoaded] = useState(false);
  const [imageError, setImageError] = useState<string | null>(null);

  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const imgARef = useRef<HTMLImageElement>(null);
  const imgBRef = useRef<HTMLImageElement>(null);
  const failCountRef = useRef(0);

  // Load photos on mount
  useEffect(() => {
    getCachedPhotos()
      .then((p) => {
        setPhotos(p);
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  }, []);

  const currentPhoto = photos.length > 0 ? photos[index] : null;

  // Try loading an image, returns true if the src was set
  const tryLoadImage = useCallback(
    (img: HTMLImageElement, photo: CachedPhoto, onSuccess: () => void) => {
      const src = convertFileSrc(photo.file_path);

      img.onload = () => {
        failCountRef.current = 0;
        setImageLoaded(true);
        setImageError(null);
        onSuccess();
      };
      img.onerror = () => {
        console.error("Failed to load image:", photo.file_path, "→", src);
        failCountRef.current += 1;

        if (failCountRef.current >= photos.length) {
          // All images failed — stop cycling and show error
          setPlaying(false);
          setImageError(
            `Unable to load cached images. The cache may need to be refreshed.`,
          );
        } else {
          // Skip this one, try the next
          onSuccess();
        }
      };
      img.src = src;
    },
    [photos.length],
  );

  // Advance to next photo with crossfade
  const advance = useCallback(
    (direction: 1 | -1 = 1) => {
      if (photos.length === 0) return;

      const nextIndex =
        (index + direction + photos.length) % photos.length;

      const incomingImg =
        activeSlide === "a" ? imgBRef.current : imgARef.current;
      if (!incomingImg) return;

      tryLoadImage(incomingImg, photos[nextIndex], () => {
        setIndex(nextIndex);
        setActiveSlide((s) => (s === "a" ? "b" : "a"));
      });
    },
    [photos, index, activeSlide, tryLoadImage],
  );

  // Auto-advance timer
  useEffect(() => {
    if (!playing || photos.length <= 1) return;

    timerRef.current = setTimeout(() => advance(1), DURATION_MS);
    return () => clearTimeout(timerRef.current);
  }, [playing, index, photos.length, advance]);

  // Set initial image on first load — try images sequentially until one works
  useEffect(() => {
    if (photos.length === 0) return;

    failCountRef.current = 0;
    let attempt = 0;

    function tryNext() {
      const img = imgARef.current;
      if (!img) return;

      if (attempt >= photos.length) {
        setImageError(
          "Unable to load any cached images. The cache may need to be refreshed.",
        );
        return;
      }
      const photo = photos[attempt];
      const src = convertFileSrc(photo.file_path);

      img.onload = () => {
        failCountRef.current = 0;
        setImageLoaded(true);
        setImageError(null);
        setIndex(attempt);
      };
      img.onerror = () => {
        console.error("Failed to load image:", photo.file_path, "\u2192", src);
        attempt++;
        tryNext();
      };
      img.src = src;
    }

    tryNext();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [photos]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-24 text-gray-400">
        Loading cached photos...
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-800 bg-red-900/30 p-4 text-red-300">
        <p className="font-medium">Error loading photos</p>
        <p className="mt-1 text-sm">{error}</p>
      </div>
    );
  }

  if (photos.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-24 text-gray-400">
        <svg
          className="mb-4 h-16 w-16 text-gray-600"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="m2.25 15.75 5.159-5.159a2.25 2.25 0 0 1 3.182 0l5.159 5.159m-1.5-1.5 1.409-1.409a2.25 2.25 0 0 1 3.182 0l2.909 2.909M3.75 21h16.5A2.25 2.25 0 0 0 22.5 18.75V5.25A2.25 2.25 0 0 0 20.25 3H3.75A2.25 2.25 0 0 0 1.5 5.25v13.5A2.25 2.25 0 0 0 3.75 21Z"
          />
        </svg>
        <h2 className="mb-2 text-lg font-medium text-gray-300">
          No Photos Cached
        </h2>
        <p className="max-w-sm text-center text-sm">
          Go to Settings and click <strong>Refresh Cache</strong> to download
          photos from iNaturalist.
        </p>
      </div>
    );
  }

  return (
    <div className="relative -mx-6 -mt-6 overflow-hidden bg-black"
         style={{ height: "calc(100vh - 49px)" }}>
      {/* Slide layers */}
      <div
        className="absolute inset-0 flex items-center justify-center"
        style={{
          opacity: activeSlide === "a" ? 1 : 0,
          transition: `opacity ${FADE_MS}ms ease-in-out`,
        }}
      >
        <img
          ref={imgARef}
          alt=""
          className="max-h-full max-w-full"
          style={{ objectFit: "contain" }}
        />
      </div>
      <div
        className="absolute inset-0 flex items-center justify-center"
        style={{
          opacity: activeSlide === "b" ? 1 : 0,
          transition: `opacity ${FADE_MS}ms ease-in-out`,
        }}
      >
        <img
          ref={imgBRef}
          alt=""
          className="max-h-full max-w-full"
          style={{ objectFit: "contain" }}
        />
      </div>

      {/* Image load error banner */}
      {imageError && (
        <div className="absolute inset-x-0 top-12 z-30 mx-auto max-w-md rounded-lg border border-amber-700/50 bg-amber-900/80 px-4 py-3 text-center text-sm text-amber-200 backdrop-blur-sm">
          {imageError}
        </div>
      )}

      {/* Loading indicator (before first image loads) */}
      {!imageLoaded && !imageError && (
        <div className="absolute inset-0 z-5 flex items-center justify-center text-gray-500">
          Loading preview...
        </div>
      )}

      {/* Attribution overlay */}
      {currentPhoto && imageLoaded && (
        <div
          className="absolute inset-x-0 bottom-0 z-10 px-6 pb-14 pt-16"
          style={{
            background:
              "linear-gradient(transparent, rgba(0, 0, 0, 0.75))",
          }}
        >
          <div className="flex flex-wrap items-baseline gap-2">
            {currentPhoto.common_name && (
              <span className="text-lg font-semibold text-white">
                {currentPhoto.common_name}
              </span>
            )}
            <span className="text-sm italic text-white/80">
              {currentPhoto.scientific_name}
            </span>
          </div>
          <div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-white/60">
            <span>&copy; {currentPhoto.creator_name}</span>
            <span className="rounded border border-white/20 px-1.5 py-px text-[11px]">
              {currentPhoto.license_display}
            </span>
            {currentPhoto.place_name && (
              <span>📍 {currentPhoto.place_name}</span>
            )}
          </div>
        </div>
      )}

      {/* Controls */}
      <div className="absolute inset-x-0 bottom-0 z-20 flex items-center justify-center gap-3 pb-3">
        <button
          onClick={() => advance(-1)}
          className="rounded-full bg-black/50 p-2 text-white/70 backdrop-blur-sm transition-colors hover:bg-black/70 hover:text-white"
          title="Previous"
        >
          <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
          </svg>
        </button>
        <button
          onClick={() => {
            setPlaying((p) => !p);
          }}
          className="rounded-full bg-black/50 p-2.5 text-white/70 backdrop-blur-sm transition-colors hover:bg-black/70 hover:text-white"
          title={playing ? "Pause" : "Play"}
        >
          {playing ? (
            <svg className="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
              <path d="M6 4h4v16H6V4zm8 0h4v16h-4V4z" />
            </svg>
          ) : (
            <svg className="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z" />
            </svg>
          )}
        </button>
        <button
          onClick={() => advance(1)}
          className="rounded-full bg-black/50 p-2 text-white/70 backdrop-blur-sm transition-colors hover:bg-black/70 hover:text-white"
          title="Next"
        >
          <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5" />
          </svg>
        </button>
      </div>

      {/* Photo counter */}
      <div className="absolute right-3 top-3 z-20 rounded-full bg-black/50 px-3 py-1 text-xs text-white/60 backdrop-blur-sm">
        {index + 1} / {photos.length}
      </div>
    </div>
  );
}
