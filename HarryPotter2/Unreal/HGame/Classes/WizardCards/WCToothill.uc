//===============================================================================
//  #89 Alberta Toothill
//===============================================================================

class  WCToothill extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardToothillTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Toothillsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardToothillBigTexture FILE=TEXTURES\menu\Folio\Cards\Toothillbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Alberta Toothill";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=89
    skin(0)=Texture'HProps.skins.WizardCardToothillTex0'
	textureBig=Texture'WizCardToothillBigTexture'
	strDescriptionId="WizCard_0003"
}
