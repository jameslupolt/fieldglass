/**
 * Field Glass — Slideshow Engine
 *
 * Shared by macOS .saver (Track A) and Windows .scr.
 *
 * Protocol:
 *   The host (Swift or Rust) provides photos by calling:
 *     window.iNatSlideshow.loadPhotos(photos)  — array of photo objects
 *     window.iNatSlideshow.start()              — begin cycling
 *     window.iNatSlideshow.stop()               — stop cycling
 *     window.iNatSlideshow.setDuration(secs)    — set photo display time
 *     window.iNatSlideshow.setFillMode(mode)    — "contain" or "fill"
 *
 *   Each photo object:
 *     {
 *       src: string,              // file:// or relative path to image
 *       commonName: string,       // e.g., "Eastern Bluebird"
 *       scientificName: string,   // e.g., "Sialia sialis"
 *       creator: string,          // e.g., "Jane Smith"
 *       license: string,          // e.g., "CC BY-NC 4.0"
 *       place: string,            // e.g., "Brooklyn, NY"
 *       isNoDerivatives: boolean  // true if ND-licensed (can't crop)
 *     }
 *
 * Animation:
 *   Uses CSS transitions exclusively. The JS only toggles classes
 *   and swaps src attributes. No requestAnimationFrame or setInterval
 *   for visual effects — the CSS `transition` property drives the
 *   crossfade. A single setTimeout drives the photo timer (or the
 *   host can drive timing via evaluateJavaScript calls).
 */

(function () {
  "use strict";

  // --- State ---
  let photos = [];
  let currentIndex = -1;
  let durationMs = 15000;
  let fillMode = "contain"; // "contain" or "fill"
  let timerId = null;
  let activeSlide = "a"; // which slide is currently visible

  // --- DOM refs ---
  const slideA = document.getElementById("slide-a");
  const slideB = document.getElementById("slide-b");
  const imgA = document.getElementById("img-a");
  const imgB = document.getElementById("img-b");
  const elCommon = document.getElementById("species-common");
  const elScientific = document.getElementById("species-scientific");
  const elCreator = document.getElementById("creator");
  const elLicense = document.getElementById("license");
  const elPlace = document.getElementById("place");

  // --- Core logic ---

  function nextPhoto() {
    if (photos.length === 0) return;

    currentIndex = (currentIndex + 1) % photos.length;
    const photo = photos[currentIndex];

    // Determine which slide is the incoming one (the hidden one)
    const incomingSlide = activeSlide === "a" ? slideB : slideA;
    const incomingImg = activeSlide === "a" ? imgB : imgA;
    const outgoingSlide = activeSlide === "a" ? slideA : slideB;

    // Set the image source on the hidden slide
    incomingImg.src = photo.src;

    // Apply fill mode (but respect ND licenses — always contain for ND)
    const effectiveMode = photo.isNoDerivatives ? "contain" : fillMode;
    incomingImg.classList.toggle("fill", effectiveMode === "fill");

    // Wait for the image to load before transitioning
    incomingImg.onload = function () {
      // Update overlay text
      elCommon.textContent = photo.commonName || "";
      elScientific.textContent = photo.scientificName || "";
      elCreator.textContent = photo.creator || "";
      elLicense.textContent = photo.license || "";
      elPlace.textContent = photo.place || "";

      // Crossfade: activate incoming, deactivate outgoing
      incomingSlide.classList.add("active");
      outgoingSlide.classList.remove("active");

      // Flip the active tracker
      activeSlide = activeSlide === "a" ? "b" : "a";

      // Schedule next transition
      scheduleNext();
    };

    // If image fails to load, skip to next
    incomingImg.onerror = function () {
      scheduleNext();
    };
  }

  function scheduleNext() {
    clearTimeout(timerId);
    timerId = setTimeout(nextPhoto, durationMs);
  }

  // --- Public API ---

  window.iNatSlideshow = {
    /**
     * Load an array of photo objects into the slideshow.
     * Does not start playback — call start() after loading.
     */
    loadPhotos: function (newPhotos) {
      photos = newPhotos || [];
      currentIndex = -1;
    },

    /**
     * Begin the slideshow. Shows the first photo immediately.
     */
    start: function () {
      if (photos.length === 0) return;
      nextPhoto();
    },

    /**
     * Stop the slideshow timer.
     */
    stop: function () {
      clearTimeout(timerId);
      timerId = null;
    },

    /**
     * Set the display duration for each photo in seconds.
     */
    setDuration: function (secs) {
      durationMs = Math.max(1, secs) * 1000;
    },

    /**
     * Set the photo fit mode: "contain" (letterbox) or "fill" (crop).
     * ND-licensed photos always use contain regardless of this setting.
     */
    setFillMode: function (mode) {
      fillMode = mode === "fill" ? "fill" : "contain";
    },

    /**
     * Advance to the next photo immediately (for testing or host-driven timing).
     */
    next: function () {
      nextPhoto();
    },
  };
})();
