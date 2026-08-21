//===============================================================================
//  #98 Dymphna Furmage
//===============================================================================

class  WCFurmage extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardFurmageTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Furmagesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardFurmageBigTexture FILE=TEXTURES\menu\Folio\Cards\Furmagebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Dymphna Furmage";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=98
    skin(0)=Texture'HProps.skins.WizardCardFurmageTex0'
	textureBig=Texture'WizCardFurmageBigTexture'
	strDescriptionId="WizCard_0012"
}
