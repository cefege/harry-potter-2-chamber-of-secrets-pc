//===============================================================================
//  #97 Alberic Grunnion
//===============================================================================

class  WCGrunnion extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardGrunnionTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Grunnionsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGrunnionBigTexture FILE=TEXTURES\menu\Folio\Cards\Grunnionbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Alberic Grunnion";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=97
    skin(0)=Texture'HProps.skins.WizardCardGrunnionTex0'
	textureBig=Texture'WizCardGrunnionBigTexture'
	strDescriptionId="WizCard_0013"
}
