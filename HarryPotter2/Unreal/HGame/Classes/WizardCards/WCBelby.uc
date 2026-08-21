//===============================================================================
// #66 Flavius Belby
//===============================================================================

class  WCBelby extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBelbyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Belbysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBelbyBigTexture FILE=TEXTURES\menu\Folio\Cards\Belbybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Flavius Belby";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=66
    skin(0)=Texture'HProps.skins.WizardCardBelbyTex0'
	textureBig=Texture'WizCardBelbyBigTexture'
	strDescriptionId="WizCard_0060"
}
