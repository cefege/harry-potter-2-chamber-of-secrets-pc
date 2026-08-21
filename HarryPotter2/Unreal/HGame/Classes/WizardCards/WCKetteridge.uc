//===============================================================================
// #49 Elladora Ketteridge
//===============================================================================

class  WCKetteridge extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardKetteridgeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Ketteridgesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardKetteridgeBigTexture FILE=TEXTURES\menu\Folio\Cards\Ketteridgebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Elladora Ketteridge";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=49
    skin(0)=Texture'HProps.skins.WizardCardKetteridgeTex0'
	textureBig=Texture'WizCardKetteridgeBigTexture'
	strDescriptionId="WizCard_0087"
}
