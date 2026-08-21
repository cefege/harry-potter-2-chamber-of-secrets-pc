//===============================================================================
//  #30 Artemisia Lufkin
//===============================================================================

class  WCLufkin extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardLufkinTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Lufkinsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardLufkinBigTexture FILE=TEXTURES\menu\Folio\Cards\Lufkinbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Artemisia Lufkin";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=30
    skin(0)=Texture'HProps.skins.WizardCardLufkinTex0'
	textureBig=Texture'WizCardLufkinBigTexture'
	strDescriptionId="WizCard_0026"
}
