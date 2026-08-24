import { motion } from "framer-motion";
import type { RecordingStatus } from "@/types";

interface AudioDotsProps {
  status: RecordingStatus;
  audioLevel: number;
  hasAudioActivity?: boolean;
  idleColor?: string;
}

// Keep in sync with AUDIO_ACTIVITY_OFF_THRESHOLD in audio.rs
const AUDIO_ACTIVITY_THRESHOLD = 25;

// All values in rem so they scale with root font-size (pill_size setting)
const DOT_H_REM = 0.3125; // 5px / 16px
const IDLE_W_REM = 0.625; // 10px / 16px
const ACTIVE_W_REM = 0.3125; // 5px / 16px
const TOTAL_W_REM = IDLE_W_REM * 3; // 1.875rem — fixed bounding box
const R_REM = DOT_H_REM / 2; // 0.15625rem

/*
 * Layout & Animation rules for three dots:
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * IDLE state:
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  The three dots join into one rectangular bar.                      │
 * │  ┌─┐┌─┐┌─┐                                                         │
 * │  │█││█││█│  No gaps; rounded left and right edges.                  │
 * │  └─┘└─┘└─┘                                                         │
 * │  0   W  2W                                                          │
 * │  The outer radius is half the dot height (R).                       │
 * └─────────────────────────────────────────────────────────────────────┘
 *   - Container width = 3 × IDLE_W
 *   - left dot:  left=0,      borderRadius=[R, 0, 0, R]
 *   - center dot: left=W,     borderRadius=0
 *   - right dot:  left=2W,    borderRadius=[0, R, R, 0]
 *   - Result: seamless rectangular bar
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * RECORDING state:
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │  The three dots separate into circles.                              │
 * │  ●      ●      ●                                                    │
 * │  ↑      ↑      ↑                                                    │
 * │  Each dot stays anchored while its width changes.                   │
 * │  The outer dots grow inward; the center dot grows symmetrically.    │
 * └─────────────────────────────────────────────────────────────────────┘
 *   - Each dot shrinks via scaleX = ACTIVE_W / IDLE_W (0.5)
 *   - All dots have full circular borderRadius: [R, R, R, R]
 *   - Gaps appear naturally from anchored scale animation
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * Animation transition:
 *   Idle → Recording: borderRadius first, then scaleX
 *   Recording → Idle: scaleX first, then borderRadius
 *   Stagger effect: left→right when entering, right→left when exiting
 * ═══════════════════════════════════════════════════════════════════════════
 */

// Idle: left-rounded | square | right-rounded (rem values for precision)
const IDLE_RADIUS = [
  `${R_REM}rem 0rem 0rem ${R_REM}rem`,
  `0rem 0rem 0rem 0rem`,
  `0rem ${R_REM}rem ${R_REM}rem 0rem`,
];
// Recording: use "50%" which is relative to element dimensions
// For 10px×5px element: horizontal radius = 5px, vertical = 2.5px
// After scaleX=0.5: horizontal = 2.5px (scaled), vertical = 2.5px (unchanged) → perfect circle!
const ACTIVE_RADIUS = "50%";

// transform-origin for each dot (controls anchor point during scaleX animation)
const TRANSFORM_ORIGIN = ["left center", "center center", "right center"];

// scaleX value: 1 = full width (IDLE), ACTIVE_W/IDLE_W = shrunk (RECORDING)
const SCALE_IDLE = 1;
const SCALE_ACTIVE = ACTIVE_W_REM / IDLE_W_REM; // 0.5

/**
 * Per-property transitions with sequenced timing:
 *
 * Idle → Recording:  borderRadius first (fast), then scale (slightly slower)
 * Recording → Idle:  scale first (fast), then borderRadius (slightly slower)
 */
function dotTransition(i: number, isRecording: boolean) {
  // Stagger: left→right when entering, right→left when exiting
  const stagger = isRecording ? i * 0.04 : (2 - i) * 0.04;
  const OFFSET = 0.07; // delay between the "first" and "second" property group

  if (isRecording) {
    // borderRadius leads, scale follows
    return {
      borderRadius: {
        duration: 0.13,
        ease: "easeOut" as const,
        delay: stagger,
      },
      scaleX: {
        duration: 0.18,
        ease: "easeOut" as const,
        delay: stagger + OFFSET,
      },
      backgroundColor: { duration: 0.28, ease: "easeOut" as const },
    };
  } else {
    // scale leads, borderRadius follows
    return {
      scaleX: { duration: 0.13, ease: "easeOut" as const, delay: stagger },
      borderRadius: {
        duration: 0.18,
        ease: "easeOut" as const,
        delay: stagger + OFFSET,
      },
      backgroundColor: { duration: 0.28, ease: "easeOut" as const },
    };
  }
}

export function AudioDots({ status, audioLevel, hasAudioActivity, idleColor = "rgba(255,255,255,0.7)" }: AudioDotsProps) {
  const isRecording = status === "recording";
  const isSttRunning = status === "transcribing" || status === "processing";
  const isPolishing = status === "polishing";
  const isError = status === "error";
  const hasAudio = hasAudioActivity ?? audioLevel > AUDIO_ACTIVITY_THRESHOLD;
  const showAnimatedState = isSttRunning || isPolishing;
  const processingColor = isPolishing
    ? ["rgb(72, 148, 255)", "rgb(214, 231, 255)", "rgb(72, 148, 255)"]
    : ["rgb(0, 170, 255)", "rgb(205, 219, 255)", "rgb(0, 170, 255)"];

  const scale = isRecording ? SCALE_ACTIVE : SCALE_IDLE;

  return (
    <div className="flex items-center justify-center w-8 h-4">
      <div
        style={{
          position: "relative",
          width: `${TOTAL_W_REM}rem`,
          height: `${DOT_H_REM}rem`,
        }}
      >
        {[0, 1, 2].map((i) => {
          // Fixed positions for seamless layout in IDLE state
          const left = i === 0 ? 0 : i === 1 ? IDLE_W_REM : IDLE_W_REM * 2;
          const baseTransition = dotTransition(i, isRecording);

          return (
            <motion.div
              key={i}
              style={{
                position: "absolute",
                top: 0,
                left: `${left}rem`,
                width: `${IDLE_W_REM}rem`, // Base width, actual size controlled by scaleX
                height: `${DOT_H_REM}rem`,
                transformOrigin: TRANSFORM_ORIGIN[i],
              }}
              animate={{
                scaleX: scale,
                borderRadius: isRecording ? ACTIVE_RADIUS : IDLE_RADIUS[i],
                backgroundColor: isError
                  ? "rgb(239, 68, 68)"
                  : showAnimatedState
                    ? processingColor
                    : isRecording
                      ? hasAudio
                        ? "rgb(3, 227, 52)"
                        : idleColor
                      : idleColor,
              }}
              transition={
                showAnimatedState
                  ? {
                      ...baseTransition,
                      backgroundColor: {
                        duration: 1.4,
                        repeat: Infinity,
                        repeatType: "reverse",
                        ease: "easeInOut",
                      },
                    }
                  : baseTransition
              }
            />
          );
        })}
      </div>
    </div>
  );
}
