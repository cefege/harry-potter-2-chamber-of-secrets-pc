/*=============================================================================
	DrawTile.cpp: Unreal XOpenGL for DrawTile routines.
	Used f.e. for HUD drawing.

	Copyright 2014-2017 Oldunreal

	Revision history:
		* Created by Smirftsch
        * Added support for bindless textures
        * removed some blending changes (PF_TwoSided PF_Unlit)
        * Added batching
=============================================================================*/

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include "XOpenGLDrv.h"
#include "XOpenGL.h"

/*-----------------------------------------------------------------------------
	RenDev Interface
-----------------------------------------------------------------------------*/

void UXOpenGLRenderDevice::DrawTile(FSceneNode* Frame, FTextureInfo& Info, FLOAT X, FLOAT Y, FLOAT XL, FLOAT YL, FLOAT U, FLOAT V, FLOAT UL, FLOAT VL, class FSpanBuffer* Span, FLOAT Z, FPlane Color, FPlane Fog, DWORD PolyFlags)
{
	if (NoDrawTile)
		return;

	STAT(clockFast(Stats.TileBufferCycles));

	SetProgram(Tile_Prog);

	if (PolyFlags & PF_Modulated)
	{
		Color.X = 1.0f;
		Color.Y = 1.0f;
		Color.Z = 1.0f;
		Color.W = 1.0f;
	}
	if (Info.Texture && Info.Texture->Alpha > 0.f)
		Color.W = Info.Texture->Alpha;
	else
		Color.W = 1.0f;

	SetTexture(DiffuseTextureIndex, Info, PolyFlags, 0);
	const FTexInfo& TextureInfo = TexInfo[DiffuseTextureIndex];
	SubmitTileBatch(
		Frame,
		TextureInfo.BindlessTexHandle,
		TextureInfo.UMult,
		TextureInfo.VMult,
		X, Y, XL, YL, U, V, UL, VL, Z, Color, PolyFlags
	);

	STAT(unclockFast(Stats.TileBufferCycles));
}

UBOOL UXOpenGLRenderDevice::DrawNativeTextTile(
	FSceneNode* Frame,
	GLuint Texture,
	GLuint Sampler,
	GLuint64 BindlessTextureHandle,
	INT TextureWidth,
	INT TextureHeight,
	FLOAT X,
	FLOAT Y,
	FLOAT XL,
	FLOAT YL,
	FLOAT U,
	FLOAT V,
	FLOAT UL,
	FLOAT VL,
	FLOAT Z,
	FPlane Color,
	DWORD PolyFlags
)
{
	if (NoDrawTile || !Frame || !Texture || !Sampler || TextureWidth <= 0 || TextureHeight <= 0)
		return 0;
	if (UsingBindlessTextures && !BindlessTextureHandle)
		return 0;

	STAT(clockFast(Stats.TileBufferCycles));
	SetProgram(Tile_Prog);

	// A tile batch records only texture handles.  It must be issued before
	// changing the texture/sampler state it relies on.
	if (NativeTextActiveTexture != Texture)
	{
		PrepareNativeTextTextureMutation(Texture);
		glBindSampler(DiffuseTextureIndex, Sampler);
		NativeTextActiveTexture = Texture;

		FTexInfo& TextureInfo = TexInfo[DiffuseTextureIndex];
		TextureInfo.UMult = 1.f / static_cast<FLOAT>(TextureWidth);
		TextureInfo.VMult = 1.f / static_cast<FLOAT>(TextureHeight);
		TextureInfo.BindlessTexHandle = BindlessTextureHandle;
	}

	SubmitTileBatch(
		Frame,
		BindlessTextureHandle,
		1.f / static_cast<FLOAT>(TextureWidth),
		1.f / static_cast<FLOAT>(TextureHeight),
		X, Y, XL, YL, U, V, UL, VL, Z, Color, PolyFlags
	);
	STAT(unclockFast(Stats.TileBufferCycles));
	return 1;
}

void UXOpenGLRenderDevice::FlushNativeTextTileBatch()
{
	if (ActiveProgram == Tile_Prog && Shaders[Tile_Prog])
		Shaders[Tile_Prog]->Flush(false);
}

void UXOpenGLRenderDevice::SubmitTileBatch(
	FSceneNode* Frame,
	GLuint64 BindlessTextureHandle,
	FLOAT UMult,
	FLOAT VMult,
	FLOAT X,
	FLOAT Y,
	FLOAT XL,
	FLOAT YL,
	FLOAT U,
	FLOAT V,
	FLOAT UL,
	FLOAT VL,
	FLOAT Z,
	FPlane Color,
	DWORD PolyFlags
)
{
	auto ShaderCore = dynamic_cast<DrawTileCoreProgram*>(Shaders[Tile_Prog]);
	auto ShaderES   = dynamic_cast<DrawTileESProgram*>  (Shaders[Tile_Prog]);

	DWORD DrawFlags = ShaderDrawFlags::DF_None;
	DWORD NextPolyFlags = GetPolyFlagsAndDrawFlags(PolyFlags, DrawFlags, 1);
	DrawFlags |= ShaderDrawFlags::DF_DiffuseTexture;
	PolyFlags &= ~(PF_RenderHint | PF_Unlit); // Using PF_RenderHint internally for CW/CCW switch.

	if (GIsEditor && NextPolyFlags & PF_Selected)
		DrawFlags |= ShaderDrawFlags::DF_Selected;

	glm::vec4 DrawColor = HitTesting() ? FPlaneToVec4(HitColor) : FPlaneToVec4(Color);
	bool CanBuffer = false;
	DrawTileParameters* DrawCallParams = nullptr;

	ShaderProgram* Shader = Shaders[Tile_Prog];
	const DWORD PerDrawOptionsMask = ShaderCompilationOptions::OPT_IsMasked | ShaderCompilationOptions::OPT_IsAlphaBlended;
	const DWORD PerDrawSignature = DrawFlags & (ShaderDrawFlags::DF_Masked | ShaderDrawFlags::DF_AlphaBlended);

	ShaderCompilationOptions RendererConfigOptions = Shader->CurrentSpecialization->Options;
	RendererConfigOptions.UnsetOption(PerDrawOptionsMask);

	ShaderCompilationOptions RequiredOptions;
	if (PerDrawSignature == Shader->LastPerDrawSignature && RendererConfigOptions == Shader->LastRendererConfigOptions)
	{
		RequiredOptions = Shader->LastResolvedOptions;
	}
	else
	{
		RequiredOptions = RendererConfigOptions;
		if (DrawFlags & ShaderDrawFlags::DF_Masked)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsMasked);
		if (DrawFlags & ShaderDrawFlags::DF_AlphaBlended)
			RequiredOptions.SetOption(ShaderCompilationOptions::OPT_IsAlphaBlended);

		Shader->LastPerDrawSignature = PerDrawSignature;
		Shader->LastRendererConfigOptions = RendererConfigOptions;
		Shader->LastResolvedOptions = RequiredOptions;
	}

	if (ShaderCore)
	{
		CanBuffer = ShaderCore->VertBuffer.CanBuffer(3) &&
			ShaderCore->ParametersBuffer.CanBuffer(1) &&
			!ShaderCore->DrawBuffer.IsFull();
	}
	else
	{
		CanBuffer = ShaderES->VertBuffer.CanBuffer(6) &&
			ShaderES->ParametersBuffer.CanBuffer(1) &&
			!ShaderES->DrawBuffer.IsFull();
	}

	if (WillBlendStateChange(CurrentBlendPolyFlags, NextPolyFlags) ||
		!(RequiredOptions == Shader->CurrentSpecialization->Options) ||
		!CanBuffer)
	{
		if (ShaderCore)
			ShaderCore->Flush(!CanBuffer);
		else
			ShaderES->Flush(!CanBuffer);
		SetBlend(NextPolyFlags);
	}

	Shader->SelectSpecialization(RequiredOptions);

	DrawCallParams = ShaderCore ? ShaderCore->ParametersBuffer.GetCurrentElementPtr() : ShaderES->ParametersBuffer.GetCurrentElementPtr();
	DrawCallParams->DrawColor = DrawColor;
	DrawCallParams->TexHandles[DiffuseTextureIndex] = BindlessTextureHandle;
	DrawCallParams->DrawFlags = DrawFlags;

	if (GIsEditor &&
		Frame->Viewport->Actor &&
		(Frame->Viewport->IsOrtho() || Abs(Z) <= SMALL_NUMBER))
	{
		Z = 1.0f;
	}

	if (ShaderES)
	{
		ShaderES->DrawBuffer.StartDrawCall();
		auto Out = ShaderES->VertBuffer.GetCurrentElementPtr();
		const auto DrawID = ShaderES->DrawBuffer.GetDrawID();

		Out[0].Coords = glm::vec3(RFX2 * Z * (X - Frame->FX2), RFY2 * Z * (Y - Frame->FY2), Z);
		Out[0].DrawID = DrawID;
		Out[0].TexCoords = glm::vec2(U * UMult, V * VMult);

		Out[1].Coords = glm::vec3(RFX2 * Z * (X + XL - Frame->FX2), RFY2 * Z * (Y - Frame->FY2), Z);
		Out[1].DrawID = DrawID;
		Out[1].TexCoords = glm::vec2((U + UL) * UMult, V * VMult);

		Out[2].Coords = glm::vec3(RFX2 * Z * (X + XL - Frame->FX2), RFY2 * Z * (Y + YL - Frame->FY2), Z);
		Out[2].DrawID = DrawID;
		Out[2].TexCoords = glm::vec2((U + UL) * UMult, (V + VL) * VMult);

		Out[3].Coords = glm::vec3(RFX2 * Z * (X - Frame->FX2), RFY2 * Z * (Y - Frame->FY2), Z);
		Out[3].DrawID = DrawID;
		Out[3].TexCoords = glm::vec2(U * UMult, V * VMult);

		Out[4].Coords = glm::vec3(RFX2 * Z * (X + XL - Frame->FX2), RFY2 * Z * (Y + YL - Frame->FY2), Z);
		Out[4].DrawID = DrawID;
		Out[4].TexCoords = glm::vec2((U + UL) * UMult, (V + VL) * VMult);

		Out[5].Coords = glm::vec3(RFX2 * Z * (X - Frame->FX2), RFY2 * Z * (Y + YL - Frame->FY2), Z);
		Out[5].DrawID = DrawID;
		Out[5].TexCoords = glm::vec2(U * UMult, (V + VL) * VMult);

		ShaderES->VertBuffer.Advance(6);
		ShaderES->DrawBuffer.EndDrawCall(6);
		ShaderES->ParametersBuffer.Advance(1);
	}
	else
	{
		ShaderCore->DrawBuffer.StartDrawCall();
		auto Out = ShaderCore->VertBuffer.GetCurrentElementPtr();
		const auto DrawID = ShaderCore->DrawBuffer.GetDrawID();

		Out->Coords = glm::vec3(X, Y, Z);
		Out->TexCoords0 = glm::vec4(RFX2, RFY2, Frame->FX2, Frame->FY2);
		Out->TexCoords1 = glm::vec4(U, V, UL, VL);
		Out->TexCoords2 = glm::vec4(XL, YL, UMult, VMult);
		(Out++)->DrawID = DrawID;
		(Out++)->DrawID = DrawID;
		Out->DrawID = DrawID;

		ShaderCore->VertBuffer.Advance(3);
		ShaderCore->DrawBuffer.EndDrawCall(3);
		ShaderCore->ParametersBuffer.Advance(1);
	}
}

/*-----------------------------------------------------------------------------
	OpenGL ES Tile Shader
-----------------------------------------------------------------------------*/

UXOpenGLRenderDevice::DrawTileESProgram::DrawTileESProgram(const TCHAR* Name, UXOpenGLRenderDevice* RenDev)
	: ShaderProgramImpl(Name, RenDev)
{
	VertexBufferSize				= DRAWTILE_SIZE * 6; // 6 vertices per draw call
	ParametersBufferSize			= DRAWTILE_SIZE;
	ParametersBufferBindingIndex	= GlobalShaderBindingIndices::TileParametersIndex;
	NumTextureSamplers				= 1;
	DrawMode						= GL_TRIANGLES;
	UseSSBOParametersBuffer			= RenDev->UsingShaderDrawParameters; // heh. You never know...
	ParametersInfo					= DrawTileParametersInfo;
	VertexShaderFunc				= &BuildVertexShader;
	GeoShaderFunc					= nullptr;
	FragmentShaderFunc				= &BuildFragmentShader;
	DepthTesting					= 0;
}

void UXOpenGLRenderDevice::DrawTileESProgram::CreateInputLayout()
{
	for (INT i = 0; i < 3; ++i)
		glEnableVertexAttribArray(i);
	glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexES), (GLvoid*)(0));
	glVertexAttribIPointer(1, 1, GL_UNSIGNED_INT, sizeof(DrawTileVertexES), (GLvoid*)(offsetof(DrawTileVertexES, DrawID)));
	glVertexAttribPointer(2, 2, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexES), (GLvoid*)(offsetof(DrawTileVertexES, TexCoords)));
	VertBuffer.SetInputLayoutCreated();
}

void UXOpenGLRenderDevice::DrawTileESProgram::DeactivateShader()
{
	ShaderProgramImpl::DeactivateShader();


	if (RenDev->UseAA && RenDev->NoAATiles)
		glEnable(GL_MULTISAMPLE);
}

void UXOpenGLRenderDevice::DrawTileESProgram::ActivateShader()
{
	ShaderProgramImpl::ActivateShader();

#if !defined(__EMSCRIPTEN__) && !__LINUX_ARM__
	if (RenDev->UseAA && RenDev->NoAATiles)
		glDisable(GL_MULTISAMPLE);
#endif

}

/*-----------------------------------------------------------------------------
	OpenGL Core Tile Shader
-----------------------------------------------------------------------------*/

UXOpenGLRenderDevice::DrawTileCoreProgram::DrawTileCoreProgram(const TCHAR* Name, UXOpenGLRenderDevice* RenDev)
	: ShaderProgramImpl(Name, RenDev)
{
	VertexBufferSize				= DRAWTILE_SIZE * 3;
	ParametersBufferSize			= DRAWTILE_SIZE;
	ParametersBufferBindingIndex	= GlobalShaderBindingIndices::TileParametersIndex;
	NumTextureSamplers				= 1;
	DrawMode						= GL_TRIANGLES;
	UseSSBOParametersBuffer			= RenDev->UsingShaderDrawParameters;
	ParametersInfo					= DrawTileParametersInfo;
	VertexShaderFunc				= &BuildVertexShader;
	GeoShaderFunc					= &BuildGeometryShader;
	FragmentShaderFunc				= &BuildFragmentShader;
	DepthTesting					= 0;
	RelevantSpecializationOptions =
		ShaderCompilationOptions::OPT_ClipDistance |
		ShaderCompilationOptions::OPT_Editor |
		ShaderCompilationOptions::OPT_SimulateMultiPass |
		ShaderCompilationOptions::OPT_GeometryShaders;
}

void UXOpenGLRenderDevice::DrawTileCoreProgram::CreateInputLayout()
{
	for (INT i = 0; i < 5; ++i)
		glEnableVertexAttribArray(i);
	glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexCore), (GLvoid*)(0));
	glVertexAttribIPointer(1, 1, GL_UNSIGNED_INT, sizeof(DrawTileVertexCore), (GLvoid*)(offsetof(DrawTileVertexCore, DrawID)));
	glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexCore), (GLvoid*)(offsetof(DrawTileVertexCore, TexCoords0)));
	glVertexAttribPointer(3, 4, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexCore), (GLvoid*)(offsetof(DrawTileVertexCore, TexCoords1)));
	glVertexAttribPointer(4, 4, GL_FLOAT, GL_FALSE, sizeof(DrawTileVertexCore), (GLvoid*)(offsetof(DrawTileVertexCore, TexCoords2)));
	VertBuffer.SetInputLayoutCreated();
}

void UXOpenGLRenderDevice::DrawTileCoreProgram::DeactivateShader()
{
	ShaderProgramImpl::DeactivateShader();


	if (RenDev->UseAA && RenDev->NoAATiles)
		glEnable(GL_MULTISAMPLE);
}

void UXOpenGLRenderDevice::DrawTileCoreProgram::ActivateShader()
{
	ShaderProgramImpl::ActivateShader();

#if !defined(__EMSCRIPTEN__) && !__LINUX_ARM__
    if (RenDev->UseAA && RenDev->NoAATiles)
        glDisable(GL_MULTISAMPLE);
#endif

}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
