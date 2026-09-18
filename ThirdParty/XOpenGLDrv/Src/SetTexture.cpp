/*=============================================================================
	SetTexture.cpp: Unreal XOpenGL Texture handling.

	Copyright 2014-2021 Oldunreal

	Revision history:
		* Created by Smirftsch
=============================================================================*/

// Include GLM
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include "XOpenGLDrv.h"
#include "XOpenGL.h"

static UBOOL IsHP2CompressedTexture(ETextureFormat Format)
{
	return Format == TEXF_DXT1;
}

static INT HP2TextureBytes(ETextureFormat Format, INT USize, INT VSize)
{
	if (Format == TEXF_DXT1)
		return Max(1, (USize + 3) / 4) * Max(1, (VSize + 3) / 4) * 8;

	const INT BytesPerPixel =
		Format == TEXF_P8 ? 1 :
		Format == TEXF_RGB16 ? 2 :
		Format == TEXF_RGB8 ? 3 : 4;
	return USize * VSize * BytesPerPixel;
}

static const TCHAR* HP2TextureFormatName(ETextureFormat Format)
{
	switch (Format)
	{
	case TEXF_P8:    return TEXT("P8");
	case TEXF_RGBA7: return TEXT("RGBA7");
	case TEXF_RGB16: return TEXT("RGB16");
	case TEXF_DXT1:  return TEXT("DXT1");
	case TEXF_RGB8:  return TEXT("RGB8");
	case TEXF_RGBA8: return TEXT("RGBA8");
	default:         return TEXT("Unknown");
	}
}

//
// stijn: Drawing a P8 texture as PF_Masked means rendering all pixels with palette index 0 as fully transparent.
// The only way to do this is to fix up these textures just before we upload them. Specifically, we can set
// alpha to 0 on all pixels that index slot 0 in the palette.
//
// The problem is that we might render the textures without PF_Masked later. Without PF_Masked, we need to actually
// pixels that index Palette[0], so we cannot set alpha to 0 on said pixels.
//
// This means we potentially need two copies of all P8 textures: one suitable for masked drawing, with all alpha values
// for Palette[0] pixels set to zero, and one suitable for non-masked drawing with all Palette[0] pixels converted
// to RGBA as-is.
//
// This function ensures the masked and non-masked copies of the texture map to different keys in the BindMap.
// We rely on the fact that the least significant bits of a CacheID are always zero. This means it is safe to reuse
// said bits as a tag.
//
#define MASKED_TEXTURE_TAG 4
static void FixCacheID(FTextureInfo& Info, DWORD PolyFlags)
{
	if ((PolyFlags & PF_Masked) && Info.Format == TEXF_P8)
	{
		Info.CacheID |= MASKED_TEXTURE_TAG;
	}
	else
	{
		Info.CacheID &= ~MASKED_TEXTURE_TAG;
	}
}

UXOpenGLRenderDevice::FCachedTexture*
UXOpenGLRenderDevice::GetCachedTextureInfo
(
	INT Multi,
	FTextureInfo& Info,
	DWORD PolyFlags,
	UBOOL& IsResidentBindlessTexture,
	UBOOL& IsBoundToTMU,
	UBOOL& IsTextureDataStale,
	UBOOL ShouldResetStaleState
)
{
	FixCacheID(Info, PolyFlags);
	FCachedTexture* Result = BindMap->Find(Info.CacheID);

	if (UsingBindlessTextures && Result && Result->BindlessTexHandle)
		IsResidentBindlessTexture = 1;

	// The texture is not bindless resident
	IsBoundToTMU = Result && TexInfo[Multi].CurrentCacheID == Info.CacheID;

	// A realtime lock marks the one cached copy stale; HP2 exposes a boolean
	// change marker rather than the donor's monotonically increasing tag.
	if (Info.bRealtimeChanged)
	{
		IsTextureDataStale = 1;
		if (Result && ShouldResetStaleState)
			++Result->RealtimeChangeCount;
	}

	return Result;
}


void UXOpenGLRenderDevice::SetNoTexture( INT Multi )
{
	guard(UXOpenGLRenderDevice::SetNoTexture);
	if( TexInfo[Multi].CurrentCacheID != 0 )
	{
		glBindTexture( GL_TEXTURE_2D, 0 );
		TexInfo[Multi].CurrentCacheID = 0;
	}
	unguard;
}

void UXOpenGLRenderDevice::SetSampler(GLuint Sampler, FTextureInfo& Info, UBOOL SkipMipmaps, UBOOL IsLightOrFogMap, UBOOL NoSmooth)
{
	guard(UXOpenGLRenderDevice::SetSampler);

	glSamplerParameteri(Sampler, GL_TEXTURE_WRAP_S, (IsLightOrFogMap || (Info.UClamp > 0 && Info.UClamp < Info.USize)) ? GL_CLAMP_TO_EDGE : GL_REPEAT);
	glSamplerParameteri(Sampler, GL_TEXTURE_WRAP_T, (IsLightOrFogMap || (Info.VClamp > 0 && Info.VClamp < Info.VSize)) ? GL_CLAMP_TO_EDGE : GL_REPEAT);

	if (NoSmooth)
	{
		glSamplerParameteri(Sampler, GL_TEXTURE_MIN_FILTER, SkipMipmaps ? GL_NEAREST : GL_NEAREST_MIPMAP_NEAREST);
		glSamplerParameteri(Sampler, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
	}
	else if (NoFiltering)
	{
		glSamplerParameteri(Sampler, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
		glSamplerParameteri(Sampler, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
	}
	else
	{
		glSamplerParameteri(Sampler, GL_TEXTURE_MIN_FILTER, SkipMipmaps ? GL_LINEAR : (UseTrilinear ? GL_LINEAR_MIPMAP_LINEAR : GL_LINEAR_MIPMAP_NEAREST));
		glSamplerParameteri(Sampler, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

		if (MaxAnisotropy != 0.f)
			glSamplerParameterf(Sampler, GL_TEXTURE_MAX_ANISOTROPY, MaxAnisotropy);

		if (LODBias != 0.f)
			glSamplerParameterf(Sampler, GL_TEXTURE_LOD_BIAS, LODBias);
	}
	unguard;
}

#if ENGINE_VERSION==1100
static FName UserInterface = FName(TEXT("UserInterface"), FNAME_Intrinsic);
#endif

UBOOL UXOpenGLRenderDevice::UploadTexture(FTextureInfo& Info, FCachedTexture* Bind, DWORD PolyFlags, UBOOL IsFirstUpload, UBOOL IsBindlessTexture, UBOOL PartialUpload, INT U, INT V, INT UL, INT VL, BYTE* TextureData)
{
	bool UnsupportedTexture = false;

	if (Info.NumMips && !Info.Mips[0])
	{
		GWarn->Logf(TEXT("Encountered texture %ls with invalid MipMaps!"), Info.Texture->GetPathName());
		Info.NumMips = 0;
		UnsupportedTexture = true;
	}
	else
	{
		// Find lowest mip level support.
		while (Bind->BaseMip<Info.NumMips && Max(Info.Mips[Bind->BaseMip]->USize, Info.Mips[Bind->BaseMip]->VSize)>MaxTextureSize)
		{
			Bind->BaseMip++;
		}

		if (Bind->BaseMip >= Info.NumMips)
		{
			GWarn->Logf(TEXT("Encountered oversize texture %ls without sufficient mipmaps."), Info.Texture->GetPathName());
			UnsupportedTexture = true;
		}
	}

	if (SupportsLazyTextures)
		Info.Load();

	Info.bRealtimeChanged = 0;

	UBOOL UnpackSRGB = UseSRGBTextures && !(PolyFlags & PF_Modulated)
#if ENGINE_VERSION==1100 // Hack for DeusExUI.UserInterface.
		&& !(Info.Texture && Info.Texture->GetOuter() && Info.Texture->GetOuter()->GetFName() == UserInterface)
#endif
		;

	// Generate the palette.
	FColor  LocalPal[256];
	FColor* Palette = Info.Palette;// ? Info.Palette : LocalPal; // Save fallback for malformed P8.
	if (Info.Format == TEXF_P8)
	{
		if (!Info.Palette)
			appErrorf(TEXT("Encountered bogus P8 texture %ls"), Info.Texture->GetFullName());

		if (PolyFlags & PF_Masked)
		{
			// kaufel: could have kept the hack to modify and reset Info.Palette[0], but opted against.
			appMemcpy(LocalPal, Info.Palette, 256 * sizeof(FColor));
			LocalPal[0] = FColor(0, 0, 0, 0);
			Palette = LocalPal;
		}
	}

	// Download the texture.
	// If !0 allocates the requested amount of memory (and frees it afterwards).
	DWORD MinComposeSize = 0;

	GLuint InternalFormat = UnpackSRGB ? GL_SRGB8_ALPHA8 : GL_RGBA8;
	GLuint SourceFormat = GL_RGBA;
	GLuint SourceType   = GL_UNSIGNED_BYTE;

	if (!SupportsS3TC && IsHP2CompressedTexture(Info.Format))
		UnsupportedTexture = true;

	// Unsupported can already be set in case of only too large mip maps available.
	if (!UnsupportedTexture)
	{
		switch (Info.Format)
		{
		case TEXF_P8:
			MinComposeSize = Info.Mips[Bind->BaseMip]->USize * Info.Mips[Bind->BaseMip]->VSize * 4;
			InternalFormat = UnpackSRGB ? GL_SRGB8_ALPHA8 : GL_RGBA8;
			SourceFormat = GL_RGBA;
			break;

		case TEXF_RGBA7:
			MinComposeSize = Info.Mips[Bind->BaseMip]->USize * Info.Mips[Bind->BaseMip]->VSize * 4;
			InternalFormat = GL_RGBA8;
			SourceFormat = GL_RGBA;
			break;

		case TEXF_RGB16:
			InternalFormat = GL_RGB;
			SourceFormat = GL_RGB;
			SourceType = GL_UNSIGNED_SHORT_5_6_5_REV;
			break;

		case TEXF_DXT1:
			InternalFormat = UnpackSRGB
				? GL_COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT
				: GL_COMPRESSED_RGBA_S3TC_DXT1_EXT;
			break;

		case TEXF_RGB8:
			InternalFormat = UnpackSRGB ? GL_SRGB8 : GL_RGB8;
			SourceFormat = GL_RGB;
			break;

		case TEXF_RGBA8:
			InternalFormat = UnpackSRGB ? GL_SRGB8_ALPHA8 : GL_RGBA8;
			SourceFormat = GL_RGBA;
			break;

		default:
			GWarn->Logf(TEXT("Unknown texture format %ls on texture %ls."), HP2TextureFormatName(Info.Format), Info.Texture ? Info.Texture->GetPathName() : TEXT("<lightmap>"));
			UnsupportedTexture = true;
			break;
		}
	}

	// If not supported make sure we have enough compose mem for a fallback texture.
	if (UnsupportedTexture)
	{
		// 256x256 RGBA texture.
		MinComposeSize = 256 * 256 * 4;
	}

	// Allocate or enlarge compose memory if needed.
	guard(AllocateCompose);
	if (MinComposeSize > ComposeSize)
	{
		Compose = (BYTE*)appRealloc(Compose, MinComposeSize, TEXT("Compose"));
		if (!Compose)
			appErrorf(TEXT("Failed to allocate memory for texture compose."));
		ComposeSize = MinComposeSize;
	}
	unguard;

	// Unpack texture data.
	INT MaxLevel = -1;
	if (PartialUpload && TextureData)
	{
		if (!IsBindlessTexture)
			glTexSubImage2D(GL_TEXTURE_2D, ++MaxLevel, U, V, UL, VL, SourceFormat, SourceType, TextureData);
		else glTextureSubImage2D(Bind->Id, ++MaxLevel, U, V, UL, VL, SourceFormat, SourceType, TextureData);

		//debugf(TEXT("Partially reuploaded texture - U %d - V %d - UL %d - VL %d - Name %ls"), U, V, UL, VL, *FObjectName(Info.Texture));
	}
	else if (!UnsupportedTexture)
	{
		guard(Unpack texture data);
		for (INT MipIndex = Bind->BaseMip; MipIndex < Info.NumMips; MipIndex++)
		{
			// Convert the mipmap.
			FMipmapBase* Mip = Info.Mips[MipIndex];
			BYTE* ImgSrc = NULL;
			GLsizei      CompImageSize = 0; // !0 also enables use of glCompressedTex[Sub]Image2D.
			GLsizei      USize = Mip->USize;
			GLsizei      VSize = Mip->VSize;

			if (Mip && Mip->DataPtr)
			{
				switch (Info.Format)
				{
				case TEXF_P8:
					guard(ConvertP8_RGBA8888);
					ImgSrc = Compose;
					{
						DWORD* Ptr = (DWORD*)Compose;
						const INT Count = USize * VSize;
						for (INT i = 0; i < Count; ++i)
							*Ptr++ = GET_COLOR_DWORD(Palette[Mip->DataPtr[i]]);
					}
					unguard;
					break;

				case TEXF_RGBA7:
				case TEXF_RGB16:
				case TEXF_RGB8:
				case TEXF_RGBA8:
					ImgSrc = Mip->DataPtr;
					break;

				case TEXF_DXT1:
					CompImageSize = HP2TextureBytes(Info.Format, USize, VSize);
					ImgSrc = Mip->DataPtr;
					break;

				default:
					appErrorf(TEXT("Unpacking unknown format %ls on %ls."), HP2TextureFormatName(Info.Format), Info.Texture ? Info.Texture->GetFullName() : TEXT("<lightmap>"));
					break;
				}
			}
			else
			{
				const TCHAR* TextureName = Info.Texture ? Info.Texture->GetFullName() : TEXT("<lightmap>");
				GWarn->Logf(TEXT("Unpacking %ls on %ls failed due to invalid data."), HP2TextureFormatName(Info.Format), TextureName);
				break;
			}

			// Upload texture.
			if (!IsFirstUpload)
			{
				if (CompImageSize)
				{
					if (!IsBindlessTexture)
						glCompressedTexSubImage2D(GL_TEXTURE_2D, ++MaxLevel, 0, 0, USize, VSize, InternalFormat, CompImageSize, ImgSrc);
					else glCompressedTextureSubImage2D(Bind->Id, ++MaxLevel, 0, 0, USize, VSize, SourceFormat, SourceType, ImgSrc);
				}
				else
				{
					if (!IsBindlessTexture)
						glTexSubImage2D(GL_TEXTURE_2D, ++MaxLevel, 0, 0, USize, VSize, SourceFormat, SourceType, ImgSrc);
					else glTextureSubImage2D(Bind->Id, ++MaxLevel, 0, 0, USize, VSize, SourceFormat, SourceType, ImgSrc);
				}
			}
			else
			{
				if (CompImageSize)
				{
					if (GenerateMipMaps)
					{
						glCompressedTexImage2D(GL_TEXTURE_2D, 0, InternalFormat, USize, VSize, 0, CompImageSize, ImgSrc);
						glGenerateMipmap(GL_TEXTURE_2D);
						MaxLevel = Info.NumMips;
						break;
					}
					else
					{
						glCompressedTexImage2D(GL_TEXTURE_2D, ++MaxLevel, InternalFormat, USize, VSize, 0, CompImageSize, ImgSrc);
					}
				}
				else
				{
					if (GenerateMipMaps)
					{
						glTexImage2D(GL_TEXTURE_2D, 0, InternalFormat, USize, VSize, 0, SourceFormat, SourceType, ImgSrc);
						glGenerateMipmap(GL_TEXTURE_2D); // generate a complete set of mipmaps for a texture object
						MaxLevel = Info.NumMips;
						break;
					}
					else
					{
						glTexImage2D(GL_TEXTURE_2D, ++MaxLevel, InternalFormat, USize, VSize, 0, SourceFormat, SourceType, ImgSrc);
					}
				}
			}
			if (GenerateMipMaps)
				break;
		}
		unguardf((TEXT("Unpacking %ls on %ls crashed due to invalid data."), HP2TextureFormatName(Info.Format), Info.Texture ? Info.Texture->GetFullName() : TEXT("<lightmap>")));

		// This should not happen. If it happens, a sanity check is missing above.
		if (!GenerateMipMaps && MaxLevel == -1)
			GWarn->Logf(TEXT("No mip map unpacked for texture %ls."), Info.Texture ? Info.Texture->GetPathName() : TEXT("<lightmap>"));
	}

	// Create and unpack a chequerboard fallback texture texture for an unsupported format.
	guard(Unsupported);
	if (UnsupportedTexture)
	{
		check(Compose);
		check(ComposeSize >= 64 * 64 * 4);
		DWORD PaletteBM[16] =
		{
			0x00000000u, 0x000000FFu, 0x0000FF00u, 0x0000FFFFu,
			0x00FF0000u, 0x00FF00FFu, 0x00FFFF00u, 0x00FFFFFFu,
			0xFF000000u, 0xFF0000FFu, 0xFF00FF00u, 0xFF00FFFFu,
			0xFFFF0000u, 0xFFFF00FFu, 0xFFFFFF00u, 0xFFFFFFFFu,
		};
		MaxLevel = 0;
		DWORD* Ptr = (DWORD*)Compose;
		for (INT i = 0; i < (256 * 256); i++)
			*Ptr++ = PaletteBM[(i / 16 + i / (256 * 16)) % 16]; //

		glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, 256, 256, 0, GL_RGBA, GL_UNSIGNED_BYTE, Compose);
	}
	unguard;

	// Set max level.
	if (IsFirstUpload || Bind->MaxLevel != MaxLevel)
	{
		Bind->MaxLevel = MaxLevel;
		glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAX_LEVEL, MaxLevel);
	}

	// Cleanup.
	if (SupportsLazyTextures)
		Info.Unload();

	return !UnsupportedTexture;
}

void UXOpenGLRenderDevice::GenerateTextureAndSampler(FCachedTexture* Bind)
{
	glGenTextures(1, &Bind->Id);

	if (!Bind->Sampler)
		glGenSamplers(1, &Bind->Sampler);
}

void UXOpenGLRenderDevice::BindTextureAndSampler(INT Multi, FCachedTexture* Bind)
{
	glActiveTexture(GL_TEXTURE0 + Multi);
	glBindTexture(GL_TEXTURE_2D, Bind->Id);
	glBindSampler(Multi, Bind->Sampler);
}

void UXOpenGLRenderDevice::SetTexture(INT Multi, FTextureInfo& Info, DWORD PolyFlags, FLOAT PanBias)
{
	guard(UXOpenGLRenderDevice::SetTexture);

	if (ActiveProgram <= No_Prog)
        return;

	// Set panning.
	FTexInfo& Tex = TexInfo[Multi];
	Tex.UPan      = Info.Pan.X + PanBias*Info.UScale;
	Tex.VPan      = Info.Pan.Y + PanBias*Info.VScale;

    // Account for all the impact on scale normalization.
	Tex.UMult = 1.f / (Info.UScale * static_cast<FLOAT>(Info.USize));
	Tex.VMult = 1.f / (Info.VScale * static_cast<FLOAT>(Info.VSize));

	STAT(clockFast(Stats.BindCycles));

	// Check if the texture is already bound to the correct TMU
	UBOOL IsResidentBindlessTexture = 0, IsBoundToTMU = 0, IsTextureDataStale = 0;
	FCachedTexture* Bind = GetCachedTextureInfo(Multi, Info, PolyFlags, IsResidentBindlessTexture, IsBoundToTMU, IsTextureDataStale, 1);

	// Bail out early if the texture is fully up-to-date
	if (Bind && (IsResidentBindlessTexture || IsBoundToTMU) && !IsTextureDataStale)
	{
		Tex.BindlessTexHandle = Bind->BindlessTexHandle;
		STAT(unclockFast(Stats.BindCycles));
		return;
	}

	// stijn: we didn't bail out above, so *something* about this texture is about to change: either
	// we're binding a different texture to TMU Multi, or we're about to re-upload new data into a
	// texture that's already bound/resident somewhere (IsTextureDataStale). Either way, any draw calls
	// we've already batched up but not yet issued were built expecting the OLD binding/contents, so
	// flush them now, before we touch anything. This covers the bindless-resident-but-stale case too,
	// which is the one case where we won't end up calling BindTextureAndSampler below at all.
	Shaders[ActiveProgram]->Flush(false);

    // Make current.
	Tex.CurrentCacheID   = Info.CacheID;

	if (!Bind)
	{
		// Figure out OpenGL-related scaling for the texture.
		Bind = &BindMap->Set( Info.CacheID, FCachedTexture() );
		memset(Bind, 0, sizeof(FCachedTexture));
	}

	UBOOL IsNewBind = Bind->Id == 0;
	if (IsNewBind)
	{
		UBOOL SkipMipmaps = (!GenerateMipMaps && Info.NumMips == 1 && !AlwaysMipmap);
		UBOOL IsLightOrFogMap = Info.Texture == NULL;
		UBOOL NoSmooth = (PolyFlags & PF_NoSmooth) && (Multi == 0);
		GenerateTextureAndSampler(Bind);
		BindTextureAndSampler(Multi, Bind);
		SetSampler(Bind->Sampler, Info, SkipMipmaps, IsLightOrFogMap, NoSmooth);
	}
	else if (Bind->BindlessTexHandle == 0)
	{
		BindTextureAndSampler(Multi, Bind);
	}

	STAT(unclockFast(Stats.BindCycles));

	// Upload if needed.
	STAT(clockFast(Stats.ImageCycles));
	if( IsNewBind || Info.bRealtimeChanged || IsTextureDataStale )
		UploadTexture(Info, Bind, PolyFlags, IsNewBind, IsResidentBindlessTexture);

    if (UsingBindlessTextures && Bind->BindlessTexHandle == 0)
    {
        guard(MakeTextureHandleResident);
		Bind->BindlessTexHandle = glGetTextureSamplerHandleARB(Bind->Id, Bind->Sampler);

        if (!Bind->BindlessTexHandle)
        {
            GWarn->Logf(TEXT("Failed to get sampler for bindless texture: %ls!"), Info.Texture ? Info.Texture->GetFullName() : TEXT("LightMap/FogMap"));
            Bind->BindlessTexHandle = 0;
        }
        else
        {
            glMakeTextureHandleResidentARB(Bind->BindlessTexHandle);
        }

        unguard;
    }
    else if (IsNewBind)
    {
        Bind->BindlessTexHandle = 0;
    }

	Tex.BindlessTexHandle = Bind->BindlessTexHandle;

    CHECK_GL_ERROR();
	STAT(unclockFast(Stats.ImageCycles));
	unguard;
}

DWORD UXOpenGLRenderDevice::GetPolyFlagsAndDrawFlags(DWORD PolyFlags, DWORD& DrawFlags, UBOOL RemoveOccludeIfSolid)
{
	if ((PolyFlags & (PF_RenderFog | PF_Translucent)) != PF_RenderFog)
		PolyFlags &= ~PF_RenderFog;

	if (!(PolyFlags & (PF_Translucent | PF_Modulated | PF_Highlighted)))
		PolyFlags |= PF_Occlude;
	else if (RemoveOccludeIfSolid)
		PolyFlags &= ~PF_Occlude;

	const DWORD RelevantPolyFlags =
		PF_Modulated | PF_RenderFog | PF_Masked | PF_Highlighted |
		PF_Unlit | PF_Translucent | PF_Environment;
	if ((CachedPolyFlags & RelevantPolyFlags) ^ (PolyFlags & RelevantPolyFlags))
	{
		DrawFlags = ShaderDrawFlags::DF_None;

		if (PolyFlags & PF_Modulated)
			DrawFlags |= ShaderDrawFlags::DF_Modulated;
		if (PolyFlags & PF_RenderFog)
			DrawFlags |= ShaderDrawFlags::DF_RenderFog;
		if (PolyFlags & PF_Masked)
			DrawFlags |= ShaderDrawFlags::DF_Masked;
		if (PolyFlags & PF_Highlighted)
			DrawFlags |= ShaderDrawFlags::DF_AlphaBlended;
		if (PolyFlags & PF_Translucent)
			DrawFlags |= ShaderDrawFlags::DF_Translucent;
		if (PolyFlags & PF_Environment)
			DrawFlags |= ShaderDrawFlags::DF_Environment;

		CachedPolyFlags = PolyFlags;
		CachedDrawFlags = DrawFlags;
	}
	else
	{
		DrawFlags = CachedDrawFlags;
	}

	return PolyFlags;
}

void UXOpenGLRenderDevice::SetBlend(DWORD PolyFlags)
{
	guard(UXOpenGLRenderDevice::SetBlend);
	STAT(clockFast(Stats.BlendCycles));

	if (HitTesting())
	{
		glBlendFunc(GL_ONE, GL_ZERO);
		CurrentBlendPolyFlags = ~0u;
		return;
	}

	const DWORD Xor = CurrentBlendPolyFlags ^ PolyFlags;
	const DWORD BlendFlags =
		PF_Invisible | PF_Translucent | PF_Modulated | PF_Highlighted |
		PF_HighShadowDetail;

	if (Xor & (BlendFlags | PF_Occlude | PF_RenderFog))
	{
		if (Xor & BlendFlags)
		{
			if ((PolyFlags & PF_LumosAffected) == PF_LumosAffected)
			{
				glBlendFunc(GL_ONE_MINUS_SRC_ALPHA, GL_SRC_ALPHA);
			}
			else if (PolyFlags & PF_Invisible)
			{
				glBlendFunc(GL_ZERO, GL_ONE);
			}
			else if (PolyFlags & PF_Highlighted)
			{
				glBlendFunc((PolyFlags & PF_Translucent) ? GL_SRC_ALPHA : GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
			}
			else if (PolyFlags & PF_Translucent)
			{
				if (SimulateMultiPass)
					glBlendFunc(GL_SRC_ALPHA, GL_SRC1_COLOR);
				else
					glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_COLOR);
			}
			else if (PolyFlags & PF_Modulated)
			{
				glBlendFunc(GL_DST_COLOR, GL_SRC_COLOR);
			}
			else
			{
				glBlendFunc(GL_ONE, GL_ZERO);
			}
		}

		if (Xor & PF_Invisible)
		{
			const GLboolean Visible = (PolyFlags & PF_Invisible) ? GL_FALSE : GL_TRUE;
			glColorMask(Visible, Visible, Visible, Visible);
		}
		if (Xor & PF_Occlude)
			glDepthMask((PolyFlags & PF_Occlude) ? GL_TRUE : GL_FALSE);

		CurrentBlendPolyFlags = PolyFlags;
	}

	STAT(unclockFast(Stats.BlendCycles));
	unguard;
}

UBOOL UXOpenGLRenderDevice::WillBlendStateChange(DWORD OldPolyFlags, DWORD NewPolyFlags)
{
	return ((OldPolyFlags ^ NewPolyFlags) &
		(PF_Translucent | PF_Modulated | PF_Invisible | PF_Highlighted |
		 PF_HighShadowDetail | PF_Occlude | PF_RenderFog | PF_Selected)) ? 1 : 0;
}

constexpr GLenum ModeList[] = { GL_LESS, GL_EQUAL, GL_LEQUAL, GL_GREATER, GL_GEQUAL, GL_NOTEQUAL, GL_ALWAYS };
BYTE UXOpenGLRenderDevice::SetZTestMode(BYTE Mode)
{
	guard(UXOpenGLRenderDevice::SetZTestMode);
	if (LastZMode == Mode || Mode > 6)
		return Mode;

	// Flush any pending render.
	auto CurrentProgram = ActiveProgram;
	SetProgram(No_Prog);
	SetProgram(CurrentProgram);

	glDepthFunc(ModeList[Mode]);
	BYTE Prev = LastZMode;
	LastZMode = Mode;
	return Prev;
	unguard;
}

// SetBlend inspired approach to handle LineFlags.
DWORD UXOpenGLRenderDevice::SetDepth(DWORD LineFlags)
{
	guard(UXOpenGLRenderDevice::SetDepth);

	// Detect change in line flags.
	DWORD Xor = CurrentLineFlags^LineFlags;
	if (Xor & LINE_DepthCued)
	{
		if (LineFlags & LINE_DepthCued)
		{
			LastZMode = ZTEST_LessEqual;
			glDepthFunc(GL_LEQUAL);
			glDepthMask(GL_TRUE);

			// Sync with SetBlend.
			CurrentBlendPolyFlags |= PF_Occlude;
		}
		else
		{
			LastZMode = ZTEST_Always;
			glDepthFunc(GL_ALWAYS);
			glDepthMask(GL_FALSE);

			// Sync with SetBlend.
			CurrentBlendPolyFlags &= ~PF_Occlude;
		}
		CurrentLineFlags = LineFlags;
	}
	return LineFlags;
	unguard;
}
