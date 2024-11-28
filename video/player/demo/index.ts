import init, { Player } from "../pkg/video_player";

await init();

const video = document.getElementById("video") as HTMLVideoElement;

function bandwidthEstimate() {
	const bandwidthEstimate = localStorage.getItem("SCUFFLE_PLAYER_bandwidth-estimate");
	if (!bandwidthEstimate) {
		return undefined;
	}

	console.log("Using bandwidth estimate from local storage", bandwidthEstimate);

	return parseFloat(bandwidthEstimate);
}

function saveBandwidthEstimate(bandwidth: number) {
	localStorage.setItem(
		"SCUFFLE_PLAYER_bandwidth-estimate",
		`${Math.min(bandwidth, 16 * 1000 * 1000)}`,
	);
}

declare global {
	interface Window {
		SCUFFLE_PLAYER: Player;
	}
}

let player: Player;

const organizationId = document.getElementById("organization-id") as HTMLInputElement;
const edgeEndpoint = document.getElementById("edge-endpoint") as HTMLInputElement;
const edgeToken = document.getElementById("edge-token") as HTMLInputElement;

const bufferSize = document.getElementById("buffer-size") as HTMLElement;
const videoTime = document.getElementById("video-time") as HTMLElement;
const videoDuration = document.getElementById("video-duration") as HTMLElement;
const latency = document.getElementById("latency") as HTMLElement;
const frameRate = document.getElementById("frame-rate") as HTMLElement;
const resolution = document.getElementById("resolution") as HTMLElement;
const droppedFrames = document.getElementById("dropped-frames") as HTMLElement;
const bandwidth = document.getElementById("bandwidth") as HTMLElement;
const variantId = document.getElementById("variant-id") as HTMLElement;
const realTime = document.getElementById("realtime") as HTMLElement;

const selectTracksDiv = document.getElementById("select-tracks") as HTMLDivElement;
const forceTracksDiv = document.getElementById("force-tracks") as HTMLDivElement;

const toggleLowLatency = document.getElementById("toggle-low-latency") as HTMLButtonElement;
const toggleAbr = document.getElementById("toggle-abr") as HTMLButtonElement;
const jumpToLive = document.getElementById("jump-to-live") as HTMLButtonElement;
const destroy = document.getElementById("destroy") as HTMLButtonElement;
const copyShareLink = document.getElementById("copy-share") as HTMLButtonElement;

function urlParams(includeTime = true) {
	const params = new URLSearchParams();
	params.set("id", roomId.value);
	params.set("type", player.roomId ? "room" : "recording");
	if (includeTime) params.set("currentTime", `${video.currentTime}`);
	params.set("organization_id", organizationId.value);
	params.set("edge_endpoint", edgeEndpoint.value);
	params.set("edge_token", edgeToken.value);
	return params;
}

copyShareLink.addEventListener("click", () => {
	const url = new URL(window.location.href);

	url.hash = urlParams().toString();

	navigator.clipboard.writeText(url.href);

	copyShareLink.innerText = "Copied!";
	copyShareLink.disabled = true;
	setTimeout(() => {
		copyShareLink.innerText = "Copy Share Link";
		copyShareLink.disabled = false;
	}, 1000);
});

let lastFrameTime = 0;
let frameCount = 0;

function initPlayer() {
	player.lowLatency = true;
	player.abrEnabled = true;

	toggleAbr.innerText = player.abrEnabled ? "Disable ABR" : "Enable ABR";
	toggleLowLatency.innerText = player.lowLatency ? "Disable Low Latency" : "Enable Low Latency";

	player.on("destroyed", () => {
		console.log("destroyed");
		destroyed = true;
	});

	player.on("error", (evt) => {
		console.error(evt);
	});

	player.on("abr", () => {
		toggleAbr.innerText = player.abrEnabled ? "Disable ABR" : "Enable ABR";
	});

	player.on("variant", (change) => {
		console.log(change);
		variantId.innerText = `${player.variantId}`;
	});

	player.on("manifestloaded", () => {
		selectTracksDiv.innerHTML = "";
		forceTracksDiv.innerHTML = "";

		player.variants.forEach((variant, idx) => {
			const button = document.createElement("button");
			button.innerText = `${variant.audio_track.name} - ${variant.video_track?.name}`;
			button.addEventListener("click", () => {
				player.nextVariantId = idx;
			});
			selectTracksDiv.appendChild(button);

			const forceButton = document.createElement("button");
			forceButton.innerText = `${variant.audio_track.name} - ${variant.video_track?.name}`;
			forceButton.addEventListener("click", () => {
				player.variantId = idx;
			});
			forceTracksDiv.appendChild(forceButton);
		});
	});

	player.on("started", () => {
		console.log("started");
	});

	player.on("stopped", () => {
		console.log("stopped");
	});

	player.on("finished", () => {
		console.log("finished");
	});

	player.on("realtime", () => {
		console.log("realtime mode changed", player.realtimeMode);
		realTime.innerText = `${player.realtimeMode}`;
	});

	player.on("visibility", () => {
		console.log("visibility changed", player.visible);
	});
}

toggleLowLatency.addEventListener("click", () => {
	player.lowLatency = !player.lowLatency;
	jumpToLive.click();
	toggleLowLatency.innerText = player.lowLatency ? "Disable Low Latency" : "Enable Low Latency";
});

toggleAbr.addEventListener("click", () => {
	player.abrEnabled = !player.abrEnabled;
	toggleAbr.innerText = player.abrEnabled ? "Disable ABR" : "Enable ABR";
});

jumpToLive.addEventListener("click", () => {
	player.toRealtime();
});

destroy.addEventListener("click", () => {
	player.destroy();
});

let destroyed = false;

const loop = () => {
	if (destroyed) return;

	if (player) {
		try {
			if (video.buffered.length) {
				let found = false;
				for (let i = 0; i < video.buffered.length; i++) {
					const start = video.buffered.start(i);
					const end = video.buffered.end(i);
					if (video.currentTime >= start && video.currentTime <= end) {
						bufferSize.innerText = `${(end - video.currentTime).toFixed(3)}`;
						found = true;
						break;
					}
				}

				if (!found) {
					bufferSize.innerText = "-1";
				}
			} else {
				bufferSize.innerText = "0";
			}

			const currentTime = video.currentTime;
			const duration = video.seekable.length ? video.seekable.end(0) : 0;

			videoTime.innerText = `${currentTime.toFixed(3)}`;
			videoDuration.innerText = `${duration.toFixed(3)}`;
			latency.innerText = `${(duration - currentTime).toFixed(3)}`;

			resolution.innerText = `${video.videoWidth}x${video.videoHeight}`;

			saveBandwidthEstimate(player.bandwidth || 0);

			const bps = (player.bandwidth || 0) / 1000;

			if (bps > 3000) {
				bandwidth.innerText = `${(bps / 1000).toFixed(2)}Mbps`;
			} else {
				bandwidth.innerText = `${bps.toFixed(2)}Kbps`;
			}

			const quality = video.getVideoPlaybackQuality();

			droppedFrames.innerText = `${quality.droppedVideoFrames}`;

			const now = performance.now();
			if (now - lastFrameTime >= 1000) {
				frameRate.innerText = `${quality.totalVideoFrames - frameCount}`;
				frameCount = quality.totalVideoFrames;
				lastFrameTime = now;
			}
		} catch (e) {
			console.error(e);
		}
	}

	window.requestAnimationFrame(loop);
};

window.requestAnimationFrame(loop);

const loadRoomButton = document.getElementById("load-room") as HTMLButtonElement;
const loadRecordingButton = document.getElementById("load-recording") as HTMLButtonElement;
const roomId = document.getElementById("video-id") as HTMLInputElement;

function loadVideo(type: string, startTime: number) {
	if (player) player.destroy();

	if (!organizationId.value) {
		alert("Organization ID is required");
		return;
	}

	if (!edgeEndpoint.value) {
		alert("Edge endpoint is required");
		return;
	}

	if (!roomId.value) {
		alert("Room/Recording ID is required");
		return;
	}

	player = new Player(video, {
		organization_id: organizationId.value,
		server: edgeEndpoint.value,
		abr_default_bandwidth: bandwidthEstimate(),
		logging_level: "info",
	});

	window.SCUFFLE_PLAYER = player;

	initPlayer();
	console.log(player);

	if (type === "room") {
		player.loadRoom(roomId.value, edgeToken.value || undefined);
	} else if (type === "recording") {
		player.loadRecording(roomId.value, edgeToken.value || undefined);
	} else {
		throw new Error("Invalid type");
	}

	video.currentTime = startTime;

	// Update URL fragment
	const url = new URL(window.location.href);

	const params = urlParams(false);

	url.hash = params.toString();

	window.history.replaceState({}, "", url.href);

	let autoPlayed = false;
	const autoPlay = setInterval(() => {
		if (autoPlayed) return;
		autoPlayed = true;
		video
			.play()
			.then(() => {
				clearInterval(autoPlay);
			})
			.catch(() => {
				autoPlayed = false;
			});
	}, 100);
}

loadRoomButton.addEventListener("click", () => {
	loadVideo("room", -1.0);
});

loadRecordingButton.addEventListener("click", () => {
	loadVideo("recording", -1.0);
});

// Get URL fragment for the predefined url
const url = new URL(window.location.href);

const urlFragment = url.hash.slice(1);
if (urlFragment) {
	// Parse the fragment as query parameters
	const params = new URLSearchParams(urlFragment);

	const type = params.get("type");
	const currentTime = params.get("currentTime") || "-1.0";

	roomId.value = params.get("id") || "";
	organizationId.value = params.get("organization_id") || "";
	edgeEndpoint.value = params.get("edge_endpoint") || "";
	edgeToken.value = params.get("edge_token") || "";

	if (roomId.value && type && organizationId.value && edgeEndpoint.value) {
		loadVideo(type, parseFloat(currentTime));
	}
}
