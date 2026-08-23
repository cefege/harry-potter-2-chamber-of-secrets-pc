/*
 * HP2 end-of-frame capture for the vendored Vulkan driver.
 *
 * Mirrors the XOpenGLDrv end-of-present capture hook (HP2_CAPTURE_FRAMES /
 * HP2_CAPTURE_MAP / HP2_CAPTURE_TICKS -> frame_%06d.png + frame_meta.json).
 * All logic lives in this HP2-owned file; the only vendored edit is the
 * guarded call in UVulkanRenderDevice::Unlock. Vendored-mod against upstream
 * pin a29e9ac0df1c60ad302d91bc3a51ab026c1a307c.
 */

#pragma once

class UVulkanRenderDevice;

// Captures one presented frame through Renderer->ReadPixels and appends it to
// the capture directory. No-op unless HP2_CAPTURE_FRAMES names a directory;
// never throws across the driver boundary.
void Hp2CaptureFrame(UVulkanRenderDevice* Renderer);
