export default function About() {
  return (
    <div className="mx-auto max-w-md space-y-6 py-8">
      <div className="text-center">
        <h1 className="text-2xl font-bold text-green-400">
          Field Glass
        </h1>
        <p className="mt-1 text-sm text-gray-400">Version 0.1.0</p>
      </div>

      <div className="space-y-4 rounded-lg border border-gray-800 bg-gray-800/50 p-6 text-sm">
        <div>
          <h2 className="mb-1 font-medium text-gray-200">Photos</h2>
          <p className="text-gray-400">
            Nature photos sourced from{" "}
            <a
              href="https://www.inaturalist.org"
              target="_blank"
              rel="noopener noreferrer"
              className="text-green-400 underline hover:text-green-300"
            >
              iNaturalist
            </a>{" "}
            under Creative Commons licenses. All photos are attributed to
            their creators.
          </p>
        </div>

        <div>
          <h2 className="mb-1 font-medium text-gray-200">License</h2>
          <p className="text-gray-400">
            This application is open source, licensed under the{" "}
            <span className="text-gray-300">MIT License</span>.
          </p>
        </div>

        <div>
          <h2 className="mb-1 font-medium text-gray-200">Source Code</h2>
          <p className="text-gray-400">
            <a
              href="https://github.com/jameslupolt/fieldglass"
              target="_blank"
              rel="noopener noreferrer"
              className="text-green-400 underline hover:text-green-300"
            >
              Field Glass on GitHub
            </a>
          </p>
        </div>

        <div>
          <h2 className="mb-1 font-medium text-gray-200">Geocoding</h2>
          <p className="text-gray-400">
            Location search powered by{" "}
            <a
              href="https://photon.komoot.io"
              target="_blank"
              rel="noopener noreferrer"
              className="text-green-400 underline hover:text-green-300"
            >
              Photon
            </a>{" "}
            and{" "}
            <a
              href="https://nominatim.openstreetmap.org"
              target="_blank"
              rel="noopener noreferrer"
              className="text-green-400 underline hover:text-green-300"
            >
              Nominatim
            </a>
            , using OpenStreetMap data.
          </p>
        </div>
      </div>
    </div>
  );
}
