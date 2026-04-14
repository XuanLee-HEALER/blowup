/** Shared media-file extension lists + filename helpers. The previous
 *  duplication between LibraryDetailTab and LibraryDarkroomTab let the
 *  two SUB_EXTS arrays drift (one had `vtt`, the other didn't). */

export const VIDEO_EXTS: readonly string[] = [
  "mp4",
  "mkv",
  "avi",
  "mov",
  "ts",
  "flv",
  "wmv",
  "webm",
  "m4v",
];

export const SUB_EXTS: readonly string[] = ["srt", "ass", "sub", "idx", "vtt"];

export const AUDIO_EXTS: readonly string[] = [
  "mp3",
  "aac",
  "flac",
  "opus",
  "m4a",
  "wav",
  "ogg",
  "ac3",
  "dts",
  "mka",
];

export function getExt(file: string): string {
  return file.split(".").pop()?.toLowerCase() ?? "";
}

export function getStem(file: string): string {
  return file.replace(/\.[^.]+$/, "");
}

export function isVideoFile(file: string): boolean {
  return VIDEO_EXTS.includes(getExt(file));
}

export function isSubtitleFile(file: string): boolean {
  return SUB_EXTS.includes(getExt(file));
}

export function isAudioFile(file: string): boolean {
  return AUDIO_EXTS.includes(getExt(file));
}
