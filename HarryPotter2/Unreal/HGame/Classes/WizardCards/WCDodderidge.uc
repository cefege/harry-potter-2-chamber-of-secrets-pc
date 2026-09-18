//===============================================================================
//  #99 Daisy Dodderidge
//===============================================================================

class  WCDodderidge extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardDodderidgeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Dodderidgesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardDodderidgeBigTexture FILE=TEXTURES\menu\Folio\Cards\Dodderidgebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Daisy Dodderidge";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=99
    skin(0)=Texture'HProps.skins.WizardCardDodderidgeTex0'
	textureBig=Texture'WizCardDodderidgeBigTexture'
	strDescriptionId="WizCard_0011"
}
