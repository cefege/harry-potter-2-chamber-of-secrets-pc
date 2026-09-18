/*=============================================================================
	DrawGouraud.cpp: Unreal XOpenGL DrawGouraud routines.
	Used for drawing meshes.

	VertLists are only supported by 227 so far, it pushes verts in a huge
	list instead of vertice by vertice. Currently this method improves
	performance 10x and more compared to unbuffered calls. Buffering
	catches up quite some.
	Copyright 2014-2021 Oldunreal

	Todo:
        * On a long run this should be replaced with a more mode
          modern mesh rendering method, but this requires also quite some
          rework in Render.dll and will be not compatible with other
          UEngine1 games.

	Revision history:
		* Created by Smirftsch
		* Added buffering to DrawGouraudPolygon
		* implemented proper usage of persistent buffers.
		* Added bindless texture support.

=============================================================================*/

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include "XOpenGLDrv.h"
#include "XOpenGL.h"

/*-----------------------------------------------------------------------------
	Helpers
-----------------------------------------------------------------------------*/

static void BufferVert(UXOpenGLRenderDevice::DrawGouraudVertex* Vert, FTransTexture* P, glm::uint DrawID)
{
	Vert->Coords		= glm::vec3(P->Point.X, P->Point.Y, P->Point.Z);
	Vert->DrawID		= DrawID;
	Vert->Normals		= glm::vec4(P->Normal.X, P->Normal.Y, P->Normal.Z, 0.f);
	Vert->TexCoords     = glm::vec2(P->U, P->V);
	Vert->LightColor	= glm::vec4(P->Light.X, P->Light.Y, P->Light.Z, P->Light.W);
	Vert->FogColor		= glm::vec4(P->Fog.X, P->Fog.Y, P->Fog.Z, P->Fog.W);
}

static void SetTextureHelper
(
	UXOpenGLRenderDevice* RenDev, 
	INT Multi, 
	UTexture* Texture, 
	FSceneNode* Frame, 
	FTEXTURE_PTR& CachedInfo,
	glm::uint64* TexHandles,
	DWORD& DrawFlags,
	DWORD AddDrawFlag
)
{
#if XOPENGL_MODIFIED_LOCK
	CachedInfo = Texture->GetTexture(INDEX_NONE, RenDev);
#else
	Texture->Lock(CachedInfo, Frame->Viewport->CurrentTime, -1, RenDev);
#endif

	RenDev->SetTexture(Multi, FTEXTURE_GET(CachedInfo), Texture->PolyFlags, 0.f);
	TexHandles[Multi] = RenDev->TexInfo[Multi].BindlessTexHandle;
	DrawFlags |= AddDrawFlag;
}

DWORD UXOpenGLRenderDevice::PrepareGouraudCall(FSceneNode* Frame, FTextureInfo& Info, DWORD PolyFlags)
{
	auto Shader = dynamic_cast<DrawGouraudProgram*>(Shaders[Gouraud_Prog]);

	// Gather options
	DWORD DrawFlags = ShaderDrawFlags::DF_None;
	DWORD NextPolyFlags = GetPolyFlagsAndDrawFlags(PolyFlags, DrawFlags, 0);
	UBOOL NoNearZ = (GUglyHackFlags & 1) != 0;
	if (GIsEditor && NextPolyFlags & PF_Selected)
		DrawFlags |= ShaderDrawFlags::DF_Selected;

	// Figure out which texture layers this mesh uses, so we can select (or lazily build) the shader
	// specialization that only contains straight-line code for those layers. See
	// ShaderProgram::SelectSpecialization.
	const bool HasBumpMapPtr = Info.Texture && Info.Texture->BumpMap;
	const bool HasBumpMap = HasBumpMapPtr && BumpMaps;
	const DWORD PerDrawOptionsMask =
		ShaderCompilationOptions::OPT_HasDetailTexture |
		ShaderCompilationOptions::OPT_HasMacroTexture |
		ShaderCompilationOptions::OPT_HasBumpMap |
		ShaderCompilationOptions::OPT_IsMasked | ShaderCompilationOptions::OPT_IsAlphaBlended |
		ShaderCompilationOptions::OPT_IsModulated | ShaderCompilationOptions::OPT_IsRenderFog |
		ShaderCompilationOptions::OPT_IsTranslucent | ShaderCompilationOptions::OPT_IsUnlit;

	const DWORD PerDrawSignature =
		(DrawFlags & (ShaderDrawFlags::DF_Masked | ShaderDrawFlags::DF_AlphaBlended | ShaderDrawFlags::DF_Modulated | ShaderDrawFlags::DF_RenderFog | ShaderDrawFlags::DF_Translucent | ShaderDrawFlags::DF_Unlit)) |
		((Info.Texture && Info.Texture->DetailTexture) ? ShaderCompilationOptions::OPT_HasDetailTexture : 0) |
		((Info.Texture && Info.Texture->MacroTexture) ? ShaderCompilationOptions::OPT_HasMacroTexture : 0) |
		(HasBumpMapPtr ? ShaderCompilationOptions::OPT_HasBumpMap : 0);

	ShaderCompilationOptions RendererConfigOptions = Shader->CurrentSpecialization->Options;
	RendererConfigOptions.UnsetOption(PerDrawOptionsMask);

	ShaderCompilationOptions RequiredOptions;
	if (PerDrawSignature == Shader->LastPerDrawSignature && RendererConfigOptions == Shader->LastRendererConfigOptions)
	{
		// Nothing that matters has changed since the last draw call -- reuse what we computed then.
		RequiredOptions = Shader->LastResolvedOptions;
	}
	else
	{
		RequiredOptions = RendererConfigOptions; // already has the per-draw bits cleared
		if (Info.Texture && Info.Texture->DetailTexture && DetailTextures)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasDetailTexture);
		if (Info.Texture && Info.Texture->MacroTexture && MacroTextures)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasMacroTexture);
		if (HasBumpMap)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasBumpMap);
		if (DrawFlags & ShaderDrawFlags::DF_Masked)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsMasked);
		if (DrawFlags & ShaderDrawFlags::DF_AlphaBlended)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsAlphaBlended);
		if (DrawFlags & ShaderDrawFlags::DF_Modulated)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsModulated);
		if (DrawFlags & ShaderDrawFlags::DF_RenderFog)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsRenderFog);
		if (DrawFlags & ShaderDrawFlags::DF_Translucent)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsTranslucent);
		if (DrawFlags & ShaderDrawFlags::DF_Unlit)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsUnlit);

		Shader->LastPerDrawSignature = PerDrawSignature;
		Shader->LastRendererConfigOptions = RendererConfigOptions;
		Shader->LastResolvedOptions = RequiredOptions;
	}

	const bool CanBuffer = !Shader->DrawBuffer.IsFull() && Shader->ParametersBuffer.CanBuffer(1);

	// Check if global blend state or the shader specialization will change. If so, we want to flush
	// any pending draw calls before we make those changes. The texture itself no longer needs to be
	// pre-checked here -- BindTextureAndSampler flushes lazily, exactly when it actually needs to
	// rebind, instead of us predicting it upfront.
	if (WillBlendStateChange(CurrentBlendPolyFlags, NextPolyFlags) || // Check if the blending mode will change
		StoredbNearZ != NoNearZ ||  // Force a flush if we're switching between NearZ and NoNearZ
		!(RequiredOptions == Shader->CurrentSpecialization->Options) || // Check if we need a different shader specialization
		!CanBuffer // Check if we have room left in the multi-draw array
	)
	{
		// Dispatch buffered data
		Shader->Flush(!CanBuffer);

		SetBlend(NextPolyFlags);

		if (NoNearZ &&
			(StoredEffectiveFovAngle != Frame->EffectiveFovAngle ||
				StoredFX != Frame->FX ||
				StoredFY != Frame->FY ||
				!StoredbNearZ))
		{
			SetProjection(Frame, 1); // TODO/FIXME: Shouldn't this second argument be !NoNearZ ?
		}
	}

	Shader->SelectSpecialization(RequiredOptions);

	DrawGouraudParameters LocalParams{};
	DrawGouraudParameters* DrawCallParams = &LocalParams;

	const FLOAT TextureAlpha = (Info.Texture && Info.Texture->Alpha > 0.f) ? Info.Texture->Alpha : 1.f;
	
	DrawCallParams->DrawColor = HitTesting() ? FPlaneToVec4(HitColor) : glm::vec4(0.f, 0.f, 0.f, TextureAlpha);

	SetTexture(DiffuseTextureIndex, Info, NextPolyFlags, 0.0);
	DrawCallParams->DiffuseInfo = glm::vec4(TexInfo[DiffuseTextureIndex].UMult, TexInfo[DiffuseTextureIndex].VMult, Info.Texture ? Info.Texture->Diffuse : 1.f, TextureAlpha);
	DrawCallParams->TexHandles[DiffuseTextureIndex] = TexInfo[DiffuseTextureIndex].BindlessTexHandle;
	DrawFlags |= ShaderDrawFlags::DF_DiffuseTexture;

	DrawCallParams->DetailMacroInfo = glm::vec4(0.f, 0.f, 0.f, 0.f);
	if (Info.Texture && Info.Texture->DetailTexture && DetailTextures)
	{
		SetTextureHelper(this, DetailTextureIndex, Info.Texture->DetailTexture, Frame, Shader->DetailTextureInfo, DrawCallParams->TexHandles, DrawFlags, ShaderDrawFlags::DF_DetailTexture);
		DrawCallParams->DetailMacroInfo.x = TexInfo[DetailTextureIndex].UMult;
		DrawCallParams->DetailMacroInfo.y = TexInfo[DetailTextureIndex].VMult;
	}

	DrawCallParams->MiscInfo = glm::vec4(0.f, 0.f, 0.f, 0.f);
	if (Info.Texture && Info.Texture->BumpMap && BumpMaps)
	{
		SetTextureHelper(this, BumpMapIndex, Info.Texture->BumpMap, Frame, Shader->BumpMapInfo, DrawCallParams->TexHandles, DrawFlags, ShaderDrawFlags::DF_BumpMap);
		DrawCallParams->MiscInfo.x = Info.Texture->BumpMap->Specular;
	}

	if (Info.Texture && Info.Texture->MacroTexture && MacroTextures)
	{
		SetTextureHelper(this, MacroTextureIndex, Info.Texture->MacroTexture, Frame, Shader->MacroTextureInfo, DrawCallParams->TexHandles, DrawFlags, ShaderDrawFlags::DF_MacroTexture);
		DrawCallParams->DetailMacroInfo.z = TexInfo[MacroTextureIndex].UMult;
		DrawCallParams->DetailMacroInfo.w = TexInfo[MacroTextureIndex].VMult;
	}

	DrawCallParams->DrawFlags = DrawFlags;

	// Every texture layer is bound now, and any flush that setting them up could possibly have
	// triggered has already happened -- safe to fetch the real ring-buffer slot and commit our staged
	// parameters into it in one shot.
	*Shader->ParametersBuffer.GetCurrentElementPtr() = LocalParams;

	return DrawFlags;
}

void UXOpenGLRenderDevice::FinishGouraudCall(FTextureInfo& Info, DWORD DrawFlags)
{
#if !XOPENGL_MODIFIED_LOCK
	auto Shader = dynamic_cast<DrawGouraudProgram*>(Shaders[Gouraud_Prog]);
	if (DrawFlags & ShaderDrawFlags::DF_DetailTexture)
		Info.Texture->DetailTexture->Unlock(Shader->DetailTextureInfo);

	if (DrawFlags & ShaderDrawFlags::DF_BumpMap)
		Info.Texture->BumpMap->Unlock(Shader->BumpMapInfo);

	if (DrawFlags & ShaderDrawFlags::DF_MacroTexture)
		Info.Texture->MacroTexture->Unlock(Shader->MacroTextureInfo);
#endif
}

/*-----------------------------------------------------------------------------
	RenDev Interface
-----------------------------------------------------------------------------*/

void UXOpenGLRenderDevice::DrawGouraudPolygon(FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, INT NumPts, DWORD PolyFlags, FSpanBuffer* Span)
{
	guard(UXOpenGLRenderDevice::DrawGouraudPolygon);

	if (NoDrawGouraud)
		return;

	auto Shader = dynamic_cast<DrawGouraudProgram*>(Shaders[Gouraud_Prog]);

    STAT(clockFast(Stats.GouraudPolyCycles));
	SetProgram(Gouraud_Prog);

	if (NumPts < 3 /*|| Frame->Recursion > MAX_FRAME_RECURSION*/) //reject invalid.
		return;

	auto InVertexCount = NumPts - 2;
	auto OutVertexCount = InVertexCount * 3;


	if (!Shader->VertBuffer.CanBuffer(OutVertexCount)) // we check the available capacity of the parameters and draw buffer elsewhere
	{
		Shader->Flush(true);

		// just in case...
		if (OutVertexCount >= Shader->VertexBufferSize)
		{
			GWarn->Logf(TEXT("DrawGouraudPolygon poly too big!"));
			return;
		}
	}

	DWORD DrawFlags = PrepareGouraudCall(Frame, Info, PolyFlags);

	Shader->DrawBuffer.StartDrawCall();
	auto Out = Shader->VertBuffer.GetCurrentElementPtr();
	const auto DrawID = Shader->DrawBuffer.GetDrawID();

	// Unfan and buffer
	for (INT i = 0; i < InVertexCount; i++)
	{
		BufferVert(Out++, Pts[0    ], DrawID);
		BufferVert(Out++, Pts[i + 1], DrawID);
		BufferVert(Out++, Pts[i + 2], DrawID);
	}

	Shader->DrawBuffer.EndDrawCall(OutVertexCount);
	Shader->VertBuffer.Advance(OutVertexCount);
	Shader->ParametersBuffer.Advance(1);

	FinishGouraudCall(Info, DrawFlags);
    STAT(unclockFast(Stats.GouraudPolyCycles));
	unguard;
}

INT UXOpenGLRenderDevice::MaxVertices()
{
	return 256;
}

void UXOpenGLRenderDevice::DrawTriangles
(
	FSceneNode* Frame,
	FTextureInfo& Info,
	FTransTexture** Pts,
	INT NumPts,
	_WORD* Indices,
	INT NumIndices,
	DWORD PolyFlags,
	FSpanBuffer* Span
)
{
	guard(UXOpenGLRenderDevice::DrawTriangles);
	(void)Span;

	if (NoDrawGouraud || !Pts || NumPts < 3)
		return;

	const UBOOL Indexed = Indices != NULL;
	const INT OutVertexCount = Indexed ? NumIndices - (NumIndices % 3) : (NumPts - 2) * 3;
	if (OutVertexCount < 3)
		return;

	for (INT i = 0; i < (Indexed ? OutVertexCount : NumPts); ++i)
	{
		const INT PointIndex = Indexed ? Indices[i] : i;
		if (PointIndex < 0 || PointIndex >= NumPts || !Pts[PointIndex])
		{
			GWarn->Logf(TEXT("XOpenGL: DrawTriangles received an invalid vertex index."));
			return;
		}
	}

	auto Shader = dynamic_cast<DrawGouraudProgram*>(Shaders[Gouraud_Prog]);
	if (OutVertexCount >= Shader->VertexBufferSize)
	{
		GWarn->Logf(TEXT("XOpenGL: DrawTriangles needs %i vertices; buffer capacity is %i."), OutVertexCount, Shader->VertexBufferSize);
		return;
	}

	STAT(clockFast(Stats.TriangleCycles));
	SetProgram(Gouraud_Prog);
	if (!Shader->VertBuffer.CanBuffer(OutVertexCount))
		Shader->Flush(true);

	PolyFlags &= ~PF_Memorized;
	const DWORD DrawFlags = PrepareGouraudCall(Frame, Info, PolyFlags);

	Shader->DrawBuffer.StartDrawCall();
	DrawGouraudVertex* Out = Shader->VertBuffer.GetCurrentElementPtr();
	const glm::uint DrawID = Shader->DrawBuffer.GetDrawID();

	if (Indexed)
	{
		for (INT i = 0; i < OutVertexCount; ++i)
			BufferVert(Out++, Pts[Indices[i]], DrawID);
	}
	else
	{
		for (INT i = 0; i < NumPts - 2; ++i)
		{
			BufferVert(Out++, Pts[0], DrawID);
			BufferVert(Out++, Pts[i + 1], DrawID);
			BufferVert(Out++, Pts[i + 2], DrawID);
		}
	}

	Shader->DrawBuffer.EndDrawCall(OutVertexCount);
	Shader->VertBuffer.Advance(OutVertexCount);
	Shader->ParametersBuffer.Advance(1);
	FinishGouraudCall(Info, DrawFlags);
	STAT(unclockFast(Stats.TriangleCycles));

	unguard;
}

/*-----------------------------------------------------------------------------
	Gouraud Mesh Shader
-----------------------------------------------------------------------------*/

UXOpenGLRenderDevice::DrawGouraudProgram::DrawGouraudProgram(const TCHAR* Name, UXOpenGLRenderDevice* RenDev)
	: ShaderProgramImpl(Name, RenDev)
{
	VertexBufferSize				= DRAWGOURAUDPOLY_SIZE * 12;
	ParametersBufferSize			= DRAWGOURAUDPOLY_SIZE;
	ParametersBufferBindingIndex	= GlobalShaderBindingIndices::GouraudParametersIndex;
	NumTextureSamplers				= 6;
	DrawMode						= GL_TRIANGLES;
	UseSSBOParametersBuffer			= RenDev->UsingShaderDrawParameters;
	ParametersInfo					= DrawGouraudParametersInfo;
	VertexShaderFunc				= &BuildVertexShader;
	GeoShaderFunc					= RenDev->UsingGeometryShaders ? &BuildGeometryShader : nullptr; // optional
	FragmentShaderFunc				= &BuildFragmentShader;
	RelevantSpecializationOptions =
		ShaderCompilationOptions::OPT_DetailTextures |
		ShaderCompilationOptions::OPT_MacroTextures |
		ShaderCompilationOptions::OPT_BumpMaps |
		ShaderCompilationOptions::OPT_HWLighting |
		ShaderCompilationOptions::OPT_DistanceFog |
		ShaderCompilationOptions::OPT_ClipDistance |
		ShaderCompilationOptions::OPT_Editor |
		ShaderCompilationOptions::OPT_SimulateMultiPass |
		ShaderCompilationOptions::OPT_GeometryShaders;
}

void UXOpenGLRenderDevice::DrawGouraudProgram::CreateInputLayout()
{
	for (INT i = 0; i < 6; ++i)
		glEnableVertexAttribArray(i);
	using Vert = DrawGouraudVertex;
	glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(Vert), (GLvoid*)(0));
	glVertexAttribIPointer(1, 1, GL_UNSIGNED_INT,   sizeof(Vert), (GLvoid*)(offsetof(Vert, DrawID)));
	glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(Vert), (GLvoid*)(offsetof(Vert, Normals)));
	glVertexAttribPointer(3, 2, GL_FLOAT, GL_FALSE, sizeof(Vert), (GLvoid*)(offsetof(Vert, TexCoords)));
	glVertexAttribPointer(4, 4, GL_FLOAT, GL_FALSE, sizeof(Vert), (GLvoid*)(offsetof(Vert, LightColor)));
	glVertexAttribPointer(5, 4, GL_FLOAT, GL_FALSE, sizeof(Vert), (GLvoid*)(offsetof(Vert, FogColor)));
	VertBuffer.SetInputLayoutCreated();
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
