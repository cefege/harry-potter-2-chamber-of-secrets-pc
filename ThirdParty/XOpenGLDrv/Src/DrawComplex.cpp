/*=============================================================================
	DrawComplex.cpp: Unreal XOpenGL DrawComplexSurface routines.
	Used for BSP drawing.

	Copyright 2014-2021 Oldunreal

	Revision history:
		* Created by Smirftsch
=============================================================================*/

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include "XOpenGLDrv.h"
#include "XOpenGL.h"

/*-----------------------------------------------------------------------------
	Helpers
-----------------------------------------------------------------------------*/

static void SetTextureHelper
(
	UXOpenGLRenderDevice* RenDev,
	INT Multi,
	FTextureInfo& Info,
	DWORD PolyFlags,
	DWORD& DrawFlags,
	DWORD AddDrawFlag,
	FLOAT PanBias,
	glm::vec4* TextureCoords,
	glm::vec4* TextureInfo,
	glm::uint64* TexHandles
)
{
	RenDev->SetTexture(Multi, Info, PolyFlags, PanBias);
	if (TextureCoords)
		*TextureCoords = glm::vec4(RenDev->TexInfo[Multi].UMult, RenDev->TexInfo[Multi].VMult, RenDev->TexInfo[Multi].UPan, RenDev->TexInfo[Multi].VPan);
	if (TextureInfo)
		*TextureInfo = glm::vec4(Info.Texture->Diffuse > 0.f ? Info.Texture->Diffuse : 1.f, Info.Texture->Specular, Info.Texture->Alpha > 0.f ? Info.Texture->Alpha : 1.f, Info.Texture->TEXTURE_SCALE_NAME);
	TexHandles[Multi] = RenDev->TexInfo[Multi].BindlessTexHandle;
	DrawFlags |= AddDrawFlag;
}

/*-----------------------------------------------------------------------------
	RenDev Interface
-----------------------------------------------------------------------------*/

void UXOpenGLRenderDevice::DrawComplexSurface(FSceneNode* Frame, FSurfaceInfo& Surface, FSurfaceFacet& Facet, DWORD PolyFlags, BYTE cAlpha)
{
	guard(UXOpenGLRenderDevice::DrawComplexSurface);

	if (NoDrawComplexSurface)
		return;

	auto Shader = dynamic_cast<DrawComplexProgram*>(Shaders[Complex_Prog]);

    STAT(clockFast(Stats.ComplexCycles));
	SetProgram(Complex_Prog);

	check(Surface.Texture);

	// Gather options
	DWORD DrawFlags = ShaderDrawFlags::DF_None;
	const DWORD NextPolyFlags = GetPolyFlagsAndDrawFlags(PolyFlags, DrawFlags, 0);
	if (GIsEditor && NextPolyFlags & PF_Selected)
		DrawFlags |= ShaderDrawFlags::DF_Selected;

	const UBOOL UseDetailTexture = Surface.DetailTexture && !Surface.FogMap && DetailTextures;
	// Figure out which texture layers this specific surface uses, so we can select (or lazily build)
	// the shader specialization that only contains straight-line code for those layers. See
	// ShaderProgram::SelectSpecialization.
	const bool HasBumpMapPtr = Surface.Texture && Surface.Texture->Texture && Surface.Texture->Texture->BumpMap;
	const bool HasBumpMap = BumpMaps && HasBumpMapPtr;
	// The subset of ShaderCompilationOptions bits that vary per-draw for Complex, as opposed to the
	// renderer-config-derived bits (which only change when RecompileShader runs on a config reload).
	const DWORD PerDrawOptionsMask =
		ShaderCompilationOptions::OPT_HasLightMap | ShaderCompilationOptions::OPT_HasFogMap |
		ShaderCompilationOptions::OPT_HasDetailTexture | ShaderCompilationOptions::OPT_HasMacroTexture |
		ShaderCompilationOptions::OPT_HasBumpMap | ShaderCompilationOptions::OPT_HasEnvironmentMap |
		ShaderCompilationOptions::OPT_HasHeightMap |
		ShaderCompilationOptions::OPT_IsMasked | ShaderCompilationOptions::OPT_IsAlphaBlended |
		ShaderCompilationOptions::OPT_IsModulated | ShaderCompilationOptions::OPT_IsTranslucent |
		ShaderCompilationOptions::OPT_IsUnlit;

	// Calculate the per-draw signature for this draw call. This is a bitmask with relevant bits from DrawFlags and the surface's texture layers,
	// which we can use to detect when a draw call is identical to the last one in terms of what shader specialization it needs.
	const DWORD PerDrawSignature =
		(DrawFlags & (ShaderDrawFlags::DF_Masked | ShaderDrawFlags::DF_AlphaBlended | ShaderDrawFlags::DF_Modulated | ShaderDrawFlags::DF_Translucent | ShaderDrawFlags::DF_Unlit)) |
		(Surface.LightMap ? ShaderCompilationOptions::OPT_HasLightMap : 0) |
		((Surface.FogMap && Surface.FogMap->Mips[0] && Surface.FogMap->Mips[0]->DataPtr) ? ShaderCompilationOptions::OPT_HasFogMap : 0) |
		(UseDetailTexture ? ShaderCompilationOptions::OPT_HasDetailTexture : 0) |
		(Surface.MacroTexture ? ShaderCompilationOptions::OPT_HasMacroTexture : 0) |
		(HasBumpMapPtr ? ShaderCompilationOptions::OPT_HasBumpMap : 0);

	// The renderer-config subset of the currently active specialization -- i.e. with the per-draw
	// bits masked out, so it's stable across specialization switches that only differ in those bits.
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
		if (Surface.LightMap)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasLightMap);
		if (Surface.FogMap && Surface.FogMap->Mips[0] && Surface.FogMap->Mips[0]->DataPtr)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasFogMap);
		if (UseDetailTexture)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasDetailTexture);
		if (Surface.MacroTexture && MacroTextures)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasMacroTexture);
		if (HasBumpMap)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_HasBumpMap);
		// Polyflag-derived render modes
		if (DrawFlags & ShaderDrawFlags::DF_Masked)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsMasked);
		if (DrawFlags & ShaderDrawFlags::DF_AlphaBlended)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsAlphaBlended);
		if (DrawFlags & ShaderDrawFlags::DF_Modulated)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsModulated);
		if (DrawFlags & ShaderDrawFlags::DF_Translucent)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsTranslucent);
		if (DrawFlags & ShaderDrawFlags::DF_Unlit)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsUnlit);

		Shader->LastPerDrawSignature = PerDrawSignature;
		Shader->LastRendererConfigOptions = RendererConfigOptions;
		Shader->LastResolvedOptions = RequiredOptions;
	}

	const bool CanBuffer = !Shader->DrawBuffer.IsFull() && Shader->ParametersBuffer.CanBuffer(1);

	// Check if this draw call will change global blend state or the shader specialization. If so, we
	// want to flush any pending draw calls before we make those changes. Per-texture-layer state
	// changes no longer need to be pre-checked here -- BindTextureAndSampler flushes lazily, exactly
	// when a texture layer actually needs to be rebound, instead of us predicting it upfront.
	if (WillBlendStateChange(CurrentBlendPolyFlags, NextPolyFlags) || // Check if the blending mode will change
		!(RequiredOptions == Shader->CurrentSpecialization->Options) || // Check if we need a different shader specialization
		!CanBuffer)
	{
		// Dispatch buffered data
		Shader->Flush(!CanBuffer);

		// Update global GL state
		SetBlend(NextPolyFlags);
	}

	// Switch to the specialization for this surface's texture layers, lazily compiling and caching it
	// if we haven't built this exact combination before this session
	Shader->SelectSpecialization(RequiredOptions);

	// Stage this draw's parameters in a local, stack-allocated struct rather than fetching the real
	// ring-buffer slot up front. Setting up textures below can trigger BindTextureAndSampler's lazy
	// flush partway through, so writing directly into the ring buffer would not be safe.
	DrawComplexParameters LocalParams{};
	DrawComplexParameters* DrawCallParams = &LocalParams;

	// Editor Support.
	if (GIsEditor)
		DrawCallParams->DrawColor = HitTesting() ? FPlaneToVec4(HitColor) : FPlaneToVec4(Surface.FlatColor.Plane());

	// Set Textures
	SetTextureHelper(this, DiffuseTextureIndex, *Surface.Texture, NextPolyFlags, DrawFlags, ShaderDrawFlags::DF_DiffuseTexture, 0.0, &DrawCallParams->DiffuseUV, Surface.Texture->Texture ? &DrawCallParams->DiffuseInfo : nullptr, DrawCallParams->TexHandles);
	if (!Surface.Texture->Texture)
		DrawCallParams->DiffuseInfo = glm::vec4(1.f, 0.f, 0.f, cAlpha / 255.0f);
	else
		DrawCallParams->DiffuseInfo.w = cAlpha / 255.0f;

	if (Surface.LightMap)
		SetTextureHelper(this, LightMapIndex, *Surface.LightMap, 0, DrawFlags, ShaderDrawFlags::DF_LightMap, -0.5, &DrawCallParams->LightMapUV, nullptr, DrawCallParams->TexHandles);

	if (Surface.FogMap && Surface.FogMap->Mips[0] && Surface.FogMap->Mips[0]->DataPtr)
		SetTextureHelper(this, FogMapIndex, *Surface.FogMap, 0, DrawFlags, ShaderDrawFlags::DF_FogMap, -0.5, &DrawCallParams->FogMapUV, nullptr, DrawCallParams->TexHandles);

	if (UseDetailTexture)
		SetTextureHelper(this, DetailTextureIndex, *Surface.DetailTexture, 0, DrawFlags, ShaderDrawFlags::DF_DetailTexture, 0.0, &DrawCallParams->DetailUV, nullptr, DrawCallParams->TexHandles);

	if (Surface.MacroTexture && MacroTextures)
		SetTextureHelper(this, MacroTextureIndex, *Surface.MacroTexture, 0, DrawFlags, ShaderDrawFlags::DF_MacroTexture, 0.0, &DrawCallParams->MacroUV, &DrawCallParams->MacroInfo, DrawCallParams->TexHandles);

	if (HasBumpMap)
	{
		Surface.Texture->Texture->BumpMap->Lock(Shader->BumpMapInfo, FTime(), 0, this);
		SetTextureHelper(this, BumpMapIndex, Shader->BumpMapInfo, 0, DrawFlags, ShaderDrawFlags::DF_BumpMap, 0.0, nullptr, &DrawCallParams->BumpMapInfo, DrawCallParams->TexHandles);
	}

	// Other draw data
	DrawCallParams->XAxis = glm::vec4(Facet.MapCoords.XAxis.X, Facet.MapCoords.XAxis.Y, Facet.MapCoords.XAxis.Z, Facet.MapCoords.XAxis | Facet.MapCoords.Origin);
	DrawCallParams->YAxis = glm::vec4(Facet.MapCoords.YAxis.X, Facet.MapCoords.YAxis.Y, Facet.MapCoords.YAxis.Z, Facet.MapCoords.YAxis | Facet.MapCoords.Origin);
	DrawCallParams->ZAxis = glm::vec4(Facet.MapCoords.ZAxis.X, Facet.MapCoords.ZAxis.Y, Facet.MapCoords.ZAxis.Z, 0.0);
	DrawCallParams->DrawFlags = DrawFlags;

	// Every texture layer is bound now, and any flush that setting them up could possibly have
	// triggered has already happened -- safe to fetch the real ring-buffer slot and commit our staged
	// parameters into it in one shot.
	*Shader->ParametersBuffer.GetCurrentElementPtr() = LocalParams;

	Shader->DrawBuffer.StartDrawCall();
	auto DrawID = Shader->DrawBuffer.GetDrawID();

	INT FacetVertexCount = 0;
	for (FSavedPoly* Poly = Facet.Polys; Poly; Poly = Poly->Next)
	{
		const INT NumPts = Poly->NumPts;
		if (NumPts < 3) //Skip invalid polygons,if any?
			continue;

		if (!Shader->VertBuffer.CanBuffer((NumPts - 2) * 3))
		{
			Shader->DrawBuffer.EndDrawCall(FacetVertexCount);
			Shader->ParametersBuffer.Advance(1); // advance so Flush automatically restores the drawcall params of the _current_ drawcall
			Shader->Flush(true);
			Shader->DrawBuffer.StartDrawCall();
			DrawID = Shader->DrawBuffer.GetDrawID();

			// just in case...
			if ((NumPts - 2) * 3 >= Shader->VertexBufferSize)
			{
				debugf(TEXT("DrawComplexSurface facet too big (facet has %d vertices - need to buffer %d points - Vertex Buffer Size is %d)!"), NumPts, (NumPts-2) * 3, Shader->VertexBufferSize);
				continue;
			}

			FacetVertexCount = 0;
		}

		FTransform** In = &Poly->Pts[0];
		auto Out = Shader->VertBuffer.GetCurrentElementPtr();

		for (INT i = 0; i < NumPts - 2; i++)
		{
			// stijn: not using the normals currently, but we're keeping them in
			// because they make our vertex data aligned to a 32 byte boundary
			(Out  )->Coords = FPlaneToVec4(In[0    ]->Point);
			(Out++)->DrawID = DrawID;
			(Out  )->Coords = FPlaneToVec4(In[i + 1]->Point);
			(Out++)->DrawID = DrawID;
			(Out  )->Coords = FPlaneToVec4(In[i + 2]->Point);
			(Out++)->DrawID = DrawID;
		}

		FacetVertexCount += (NumPts - 2) * 3;
		Shader->VertBuffer.Advance((NumPts - 2) * 3);
	}

	Shader->DrawBuffer.EndDrawCall(FacetVertexCount);
	Shader->ParametersBuffer.Advance(1);

	if (DrawFlags & ShaderDrawFlags::DF_BumpMap)
		Surface.Texture->Texture->BumpMap->Unlock(Shader->BumpMapInfo);

    STAT(unclockFast(Stats.ComplexCycles));

	unguard;
}


/*-----------------------------------------------------------------------------
	Complex Surface Shader
-----------------------------------------------------------------------------*/

UXOpenGLRenderDevice::DrawComplexProgram::DrawComplexProgram(const TCHAR* Name, UXOpenGLRenderDevice* RenDev)
	: ShaderProgramImpl(Name, RenDev)
{
	VertexBufferSize				= DRAWCOMPLEX_SIZE * 12;
	ParametersBufferSize			= DRAWCOMPLEX_SIZE;
	ParametersBufferBindingIndex	= GlobalShaderBindingIndices::ComplexParametersIndex;
	NumTextureSamplers				= 8;
	DrawMode						= GL_TRIANGLES;
	UseSSBOParametersBuffer			= RenDev->UsingShaderDrawParameters;
	ParametersInfo					= DrawComplexParametersInfo;
	VertexShaderFunc				= &BuildVertexShader;
	GeoShaderFunc					= nullptr;
	FragmentShaderFunc				= &BuildFragmentShader;
	RelevantSpecializationOptions =
		ShaderCompilationOptions::OPT_DetailTextures |
		ShaderCompilationOptions::OPT_MacroTextures |
		ShaderCompilationOptions::OPT_EnvironmentMaps |
		ShaderCompilationOptions::OPT_BumpMaps |
		ShaderCompilationOptions::OPT_HeightMaps |
		ShaderCompilationOptions::OPT_HWLighting |
		ShaderCompilationOptions::OPT_DistanceFog |
		ShaderCompilationOptions::OPT_ClipDistance |
		ShaderCompilationOptions::OPT_Editor |
		ShaderCompilationOptions::OPT_SimulateMultiPass;
}

void UXOpenGLRenderDevice::DrawComplexProgram::CreateInputLayout()
{
	for (INT i = 0; i < 3; ++i)
		glEnableVertexAttribArray(i);
	glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(DrawComplexVertex), (GLvoid*)(0));
	glVertexAttribIPointer(1, 1, GL_UNSIGNED_INT,   sizeof(DrawComplexVertex), (GLvoid*)(offsetof(DrawComplexVertex, DrawID)));
	glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(DrawComplexVertex), (GLvoid*)(offsetof(DrawComplexVertex, Normal)));
	VertBuffer.SetInputLayoutCreated();
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
