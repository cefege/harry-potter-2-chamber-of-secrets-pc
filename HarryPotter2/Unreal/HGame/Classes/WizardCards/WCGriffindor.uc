//===============================================================================
//  #41 Godric Griffindor
//===============================================================================

class  WCGriffindor extends goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardGryffindorTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Gryffindorsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGryffindorBigTexture FILE=TEXTURES\menu\Folio\Cards\Gryffindorbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardGodricBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\41_Godric_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardGodricBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\41_Godric_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardGodricBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\41_Godric_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Godric Gryffindor";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=41
    skin(0)=Texture'HProps.skins.WizardCardGryffindorTex0'
	textureBig=Texture'WizCardGryffindorBigTexture'
	strDescriptionId="WizCard_0048"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardGodricBigTextureLayer0';
	textureLayers(1)=Texture'WizCardGodricBigTextureLayer1';
	textureLayers(2)=Texture'HPParticle.godricfire2';
	bLastLayerIsFire=true;
}

