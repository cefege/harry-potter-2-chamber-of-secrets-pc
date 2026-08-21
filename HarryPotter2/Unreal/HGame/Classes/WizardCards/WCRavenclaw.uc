//===============================================================================
//  #82 Rowena Ravenclaw
//===============================================================================

class  WCRavenclaw extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardRavenclawTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Ravenclawsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardRavenclawBigTexture FILE=TEXTURES\menu\Folio\Cards\Ravenclawbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardRowenaBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\82_Rowena_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardRowenaBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\82_Rowena_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardRowenaBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\82_Rowena_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Rowena Ravenclaw";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=82
    skin(0)=Texture'HProps.skins.WizardCardRavenclawTex0'
	textureBig=Texture'WizCardRavenclawBigTexture'
	strDescriptionId="WizCard_0076"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardRowenaBigTextureLayer0';
	textureLayers(1)=Texture'WizCardRowenaBigTextureLayer1';
	textureLayers(2)=Texture'WizCardRowenaBigTextureLayer2';

}
