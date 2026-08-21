//===============================================================================
// #29 Archibald Alderton
//===============================================================================

class  WCAlderton extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardAldertonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Aldertonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardAldertonBigTexture FILE=TEXTURES\menu\Folio\Cards\Aldertonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Archibald Alderton";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=29
    skin(0)=Texture'HProps.skins.WizardCardAldertonTex0'
	textureBig=Texture'WizCardAldertonBigTexture'
	strDescriptionId="WizCard_0036"
}
