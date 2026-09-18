//===============================================================================
//   #62 Ignatia Wildsmith
//===============================================================================

class  WCWildsmith extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardWildsmithTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wildsmithsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWildsmithBigTexture FILE=TEXTURES\menu\Folio\Cards\Wildsmithbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Ignatia Wildsmith";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=62
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE110"
    skin(0)=Texture'HProps.skins.WizardCardWildsmithTex0'
	textureBig=Texture'WizCardWildsmithBigTexture'
	strDescriptionId="WizCard_0098"
}
